use failure::{err_msg, Fallible};
use regex::RegexSet;
use reqwest::Client;

use crate::subscription::{SubjectState, Subscription, ThreadID};
use crate::SubjectType;

pub fn read_config(filename: &str) -> Fallible<String> {
    let path = dirs::home_dir()
        .ok_or_else(|| err_msg("Failed to read ~/"))?
        .join(".ghnf")
        .join(filename);
    std::fs::read_to_string(path).map_err(Into::into)
}

pub fn compile_regex() -> Fallible<RegexSet> {
    RegexSet::new(
        read_config("filters")
            .expect("Failed to read filters from ~/.ghnf/filters")
            .split('\n')
            .filter_map(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(String::from("(?i)") + s)
                }
            }),
    )
    .map_err(Into::into)
}

pub fn create_client() -> Fallible<Client> {
    let token = read_config("token")
        .expect("Failed to read GitHub token from ~/.ghnf/token")
        .split('\n')
        .next()
        .ok_or_else(|| err_msg("Malformed GitHub Personal Access Token"))?
        .to_owned();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("token {}", token))?,
    );
    Client::builder()
        .user_agent("GitHub Notification Filter (by equal-l2)")
        .default_headers(headers)
        .build()
        .map_err(Into::into)
}

pub fn load_ignored() -> Fallible<Vec<ThreadID>> {
    // `ignore` is optional, return empty vec when not found
    read_config("ignore")
        .or_else(|e| -> _ {
            if let Some(i) = e.as_fail().downcast_ref::<std::io::Error>() {
                match i.kind() {
                    std::io::ErrorKind::NotFound => Ok("".into()),
                    _ => Err(e),
                }
            } else {
                Err(e)
            }
        })?
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<ThreadID>().map_err(Into::into))
        .collect()
}

pub fn filter_ignored(ss: Vec<Subscription>) -> Fallible<Vec<Subscription>> {
    let ignore = load_ignored().expect("Failed to read GitHub token from ~/.ghnf/ignored");
    Ok(ss
        .into_iter()
        .filter(|s| !ignore.contains(&s.thread_id))
        .collect())
}

pub async fn filter_by_subject_state(
    ss: Vec<Subscription>,
    state: SubjectState,
    c: &Client,
) -> Fallible<Vec<Subscription>> {
    let mut futs = vec![];
    for s in ss {
        futs.push(async {match s.get_subject_state(c).await? {
            Some(i) if i == state => Ok(Some(s)),
            None => Ok(Some(s)), // commits doesn't have state, but we want to handle them
            _ => Ok(None)
        }});
    }
    futures::future::try_join_all(futs).await.map(|v| v.into_iter().flatten().collect())
}

pub async fn filter_and_unsubscribe(
    ss: Vec<Subscription>,
    confirm: bool,
    c: &Client,
) -> Fallible<()> {
    println!("Filtering out open notifications...");
    let candidates: Vec<Subscription> =
        filter_by_subject_state(filter_ignored(ss).unwrap(), SubjectState::Closed, c).await?;
    println!("{} notification(s) left", candidates.len());

    if candidates.is_empty() {
        println!("No notification matched");
    } else {
        if confirm {
            for s in &candidates {
                println!("{}", s);
            }

            println!("\nTo unsubscribe the notification(s), press Enter\n(If you don't want to, just abort (e.g. Ctrl+C))");
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s)?;
        }

        println!("Unsubscribing notifications...");
        let mut futs = vec![];
        for s in candidates {
            futs.push(async move {
                s.unsubscribe(c).await?;
                s.mark_as_read(c).await?;
                println!("Unsubscribed {}", s);
                Fallible::Ok(())
            });
        }
        futures::future::try_join_all(futs).await?;
    }
    Ok(())
}

pub async fn fetch_filtered(
    re: Option<&RegexSet>,
    n: Option<usize>,
    k: Option<SubjectType>,
    c: &Client,
) -> Fallible<Vec<Subscription>> {
    println!("Fetching notifications...");

    let ss = Subscription::fetch_unread(c).await?;
    println!("Fetched {} notifications", ss.len());

    let ss = if let Some(r) = re {
        println!("Filtering notifications by regex...");
        let mut futs = vec![];
        for s in ss {
            futs.push(async {
                if r.is_match(&s.subject.title) {
                    Some(s)
                } else {
                    None
                }
            });
        }
        futures::future::join_all(futs).await.into_iter().flatten().collect()
    } else {
        ss
    };

    // filter by kind
    let ss = if let Some(i) = k {
        println!("Filtering notifications by kind...");
        ss.into_iter().filter(|s| s.subject.r#type == i).collect()
    } else {
        ss
    };

    if let Some(i) = n {
        Ok(ss.into_iter().take(i).collect())
    } else {
        Ok(ss)
    }
}
