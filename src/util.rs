use failure::{err_msg, Fallible};
use rayon::prelude::*;
use regex::Regex;
use reqwest::blocking::Client;

use crate::subscription::{SubjectState, Subscription, ThreadID};
use crate::SubjectType;

pub fn read_config(filename: &str) -> Fallible<String> {
    let path = dirs::home_dir()
        .ok_or(err_msg("Failed to read ~/"))?
        .join(".ghnf")
        .join(filename);
    std::fs::read_to_string(path).map_err(Into::into)
}

pub fn compile_regex() -> Fallible<Regex> {
    let filters: Vec<_> = read_config("filters")
        .expect("Failed to read filters from ~/.ghnf/filters")
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let filters_string = String::from(r"(?i)") + &filters.join("|");
    Regex::new(&filters_string).map_err(Into::into)
}

pub fn create_client() -> Fallible<Client> {
    let token = read_config("token")
        .expect("Failed to read GitHub token from ~/.ghnf/token")
        .split('\n')
        .next()
        .ok_or(err_msg("Malformed GitHub Personal Access Token"))?
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
        .into_par_iter()
        .filter(|s| !ignore.contains(&s.thread_id))
        .collect())
}

pub fn filter_by_subject_state(
    ss: Vec<Subscription>,
    state: SubjectState,
    c: &Client,
) -> Fallible<Vec<Subscription>> {
    ss.into_par_iter()
        .map(|s| -> _ {
            Ok(match s.get_subject_state(c)? {
                Some(i) => {
                    if i == state {
                        Some(s)
                    } else {
                        None
                    }
                }
                _ => None,
            })
        })
        .filter_map(Fallible::transpose)
        .collect()
}

pub fn filter_and_unsubscribe(ss: Vec<Subscription>, confirm: bool, c: &Client) -> Fallible<()> {
    println!("Filtering out open notifications...");
    let candidates: Vec<Subscription> =
        filter_by_subject_state(filter_ignored(ss).unwrap(), SubjectState::Closed, c)?;
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
        candidates
            .into_par_iter()
            .map(|s| -> _ {
                s.unsubscribe(c)?;
                s.mark_as_read(c)?;
                println!("Unsubscribed {}", s);
                Ok(())
            })
            .collect::<Fallible<_>>()?;
    }

    Ok(())
}

pub fn fetch_filtered(
    re: &Regex,
    n: Option<usize>,
    k: Option<SubjectType>,
    c: &Client,
) -> Fallible<Vec<Subscription>> {
    println!("Fetching notifications...");
    let ss = Subscription::fetch_unread(c)?;
    println!("Fetched {} notifications", ss.len());
    println!("Filtering notifications by regex...");
    let it = if let Some(i) = k {
        ss.into_par_iter()
            .filter(|s| s.subject.r#type == i)
            .filter(|s| re.is_match(&s.subject.title))
            .collect::<Vec<_>>()
            .into_iter()
    } else {
        ss.into_par_iter()
            .filter(|s| re.is_match(&s.subject.title))
            .collect::<Vec<_>>()
            .into_iter()
    };
    if let Some(i) = n {
        Ok(it.take(i).collect())
    } else {
        Ok(it.collect())
    }
}
