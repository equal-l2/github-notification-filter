use failure::{err_msg, Fallible};
use futures::future::try_join_all;
use regex::RegexSet;
use reqwest::Client;

use crate::subscription::{SubjectState, Subscription, ThreadID};
use crate::SubjectType;

pub struct Filters {
    regex: Option<RegexSet>,
    kind: Option<SubjectType>,
    count: Option<usize>,
}

impl Filters {
    pub fn new(m: &clap::ArgMatches<'_>, with_default_regex: bool) -> Fallible<Self> {
        Ok(Self {
            regex: {
                if let Some(i) = m.value_of("filter") {
                    Some(RegexSet::new(&[i])?)
                } else if with_default_regex {
                    Some(compile_regex()?)
                } else {
                    None
                }
            },
            kind: m.value_of("kind").and_then(|v| match v {
                "commit" => Some(SubjectType::Commit),
                "issue" => Some(SubjectType::Issue),
                "pr" => Some(SubjectType::PullRequest),
                _ => unreachable!(),
            }),
            count: m.value_of("count").map(|v| {
                v.parse().unwrap_or_else(|_| {
                    eprintln!("Invalid argument for <count>, expected integer");
                    std::process::exit(1)
                })
            }),
        })
    }
}

const CHUNK_SIZE: usize = 64;

pub fn read_config(filename: &str) -> Fallible<String> {
    let path = dirs::home_dir()
        .ok_or_else(|| err_msg("Failed to read ~/"))?
        .join(".ghnf")
        .join(filename);
    std::fs::read_to_string(path).map_err(Into::into)
}

pub fn compile_regex() -> Fallible<RegexSet> {
    regex::RegexSetBuilder::new(
        read_config("filters")
            .expect("Failed to read filters from ~/.ghnf/filters")
            .split('\n')
            .filter(|s| !s.is_empty()),
    )
    .case_insensitive(true)
    .build()
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
        .pool_idle_timeout(std::time::Duration::from_secs(30)) // ++ritual++ for retrying
        .build()
        .map_err(Into::into)
}

pub fn load_ignored() -> Fallible<Vec<ThreadID>> {
    // `ignore` is optional, return empty vec when not found
    read_config("ignore")
        .or_else(|e| {
            if let Some(i) = e.as_fail().downcast_ref::<std::io::Error>() {
                if matches!(i.kind(), std::io::ErrorKind::NotFound) {
                    return Ok("".into());
                }
            }
            Err(e)
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
    let mut futs = Vec::with_capacity(CHUNK_SIZE);
    let mut ret = vec![];
    for s in ss {
        futs.push(async {
            match s.subject.r#type {
                // Commits don't have state but we want to handle them
                SubjectType::Commit => Ok(Some(s)),
                // Just ignore discussions at this moment (until GitHub fixes FIXME!)
                SubjectType::Discussion => Ok(None),
                // Now look at the state
                _ => match s.subject_state(c).await? {
                    Some(i) if i == state => Ok(Some(s)),
                    _ => Ok(None),
                },
            }
        });
        if futs.len() >= CHUNK_SIZE {
            //eprintln!("join!");
            let r: Fallible<_> = try_join_all(futs.drain(..)).await;
            ret.extend(r?.into_iter().flatten());
        }
    }
    {
        //eprintln!("final join!");
        let r: Fallible<_> = try_join_all(futs).await;
        ret.extend(r?.into_iter().flatten());
    }
    Ok(ret)
}

pub async fn unsubscribe_all(ss: Vec<Subscription>, dry: bool, c: &Client) -> Fallible<()> {
    if ss.is_empty() {
        println!("No notification matched");
        return Ok(());
    }

    if dry {
        println!("\nFollowing threads are going to be unsubscribed:");
        for s in &ss {
            println!("{}", s);
        }
        return Ok(());
    }

    println!("Unsubscribing notifications...");
    let mut futs = vec![];
    for s in ss {
        futs.push(async move {
            s.unsubscribe(c).await?;
            s.mark_as_read(c).await?;
            println!("Unsubscribed {}", s);
            Fallible::Ok(())
        });
        if futs.len() >= CHUNK_SIZE {
            try_join_all(futs.drain(..)).await?;
        }
    }
    try_join_all(futs).await?;
    Ok(())
}

pub async fn fetch_filtered(filt: Filters, c: &Client) -> Fallible<Vec<Subscription>> {
    println!("Fetching notifications...");

    let svec = Subscription::fetch_unread(c).await?;
    println!(
        "Fetched {} notifications",
        svec.iter().map(Vec::len).sum::<usize>()
    );

    let ss: Vec<Subscription> = if let Some(r) = filt.regex {
        println!("Filtering notifications by regex...");
        let mut futs = vec![];
        for ss in svec {
            futs.push(async {
                ss.into_iter().flat_map(|s| {
                    if r.is_match(&s.subject.title) {
                        Some(s)
                    } else {
                        None
                    }
                })
            });
        }
        futures::future::join_all::<Vec<_>>(futs)
            .await
            .into_iter()
            .flatten()
            .collect()
    } else {
        svec.into_iter().flatten().collect()
    };

    // filter by kind
    let ss = if let Some(i) = filt.kind {
        println!("Filtering notifications by kind...");
        ss.into_iter().filter(|s| s.subject.r#type == i).collect()
    } else {
        ss
    };

    // handle count
    let ss = if let Some(i) = filt.count {
        ss.into_iter().take(i).collect()
    } else {
        ss
    };

    Ok(ss)
}

pub fn get_last_page(link: &str) -> usize {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static R_LINK: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"<(?P<uri>[^>]*)>;\srel="(?P<rel>[^"]*)"#).unwrap());
    static R_PAGE: Lazy<Regex> = Lazy::new(|| Regex::new(r#".*\?page=(?P<page>.*)"#).unwrap());

    let last_uri = {
        let mut map = std::collections::HashMap::new();
        // Parse link header and retrieve rels
        // This parses link header with the regex below:
        // LINK ::= "<" uri ">;" WS "rel=\"" relation "\""
        for cap in R_LINK.captures_iter(link) {
            map.insert(cap["rel"].to_owned(), cap["uri"].to_owned());
        }
        map.remove("last").unwrap()
    };

    R_PAGE.captures(&last_uri).unwrap()["page"].parse().unwrap()
}
