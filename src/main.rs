use failure::{Fail, Fallible};
use reqwest::Client;
use serde_json::json;
use std::io::{BufRead, BufReader};

#[derive(Clone, Debug, serde::Deserialize)]
struct Notification {
    subject: Subject,
    url: String,
    // not currently used
    /*
    id: String,
    last_read_at: Option<String>,
    reason: String,
    repository: serde_json::Value,
    subscription_url: String,
    unread: bool,
    updated_at: Option<String>,
    */
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Subject {
    title: String,
    url: String,
    r#type: SubjectType,
    // not currently used
    /*
    latest_comment_url: Option<String>,
    */
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(field_identifier)]
enum SubjectType {
    Issue,
    PullRequest,
    Commit,
    Other(String),
}

impl std::fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SubjectType::Issue => write!(f, "Issue"),
            SubjectType::PullRequest => write!(f, "Pull Request"),
            SubjectType::Commit => write!(f, "Commit"),
            SubjectType::Other(i) => write!(f, "\"{}\"", i),
        }
    }
}

#[derive(Clone, Debug)]
struct Subscription {
    subject: Subject,
    thread_id: String, //TODO: Replace with primitive numeric type
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.url.split('/').last().unwrap().into(),
        }
    }
}

#[derive(Debug, Fail)]
enum ErrorKind {
    #[fail(display = "Unexpected response status : {}", _0)]
    ResponseStatusError(reqwest::StatusCode),
    #[fail(display = "Regex error : {}", _0)]
    RegexError(regex::Error),
    #[fail(display = "Reqwest error : {}", _0)]
    ReqwestError(reqwest::Error),
}

fn get_notification_subscriptions(client: &Client) -> Fallible<Vec<Subscription>> {
    let mut resp = client.get("https://api.github.com/notifications").send()?;

    if resp.status() != 200 {
        return Err(ErrorKind::ResponseStatusError(resp.status()).into());
    }

    let mut ss = {
        let ns: Vec<Notification> = resp.json()?;
        ns.into_iter().map(Subscription::from).collect::<Vec<_>>()
    };

    for i in 2.. {
        let mut resp = client
            .get("https://api.github.com/notifications")
            .query(&[("page", &i.to_string())])
            .send()?;
        if resp.status() == 200 {
            let ns: Vec<Notification> = resp.json()?;
            if !ns.is_empty() {
                ss.extend(ns.into_iter().map(Subscription::from));
                continue;
            }
        }
        return Ok(ss);
    }
    unreachable!();
}

fn unsubscribe_thread(client: &Client, thread_id: &String) -> Fallible<()> {
    let resp = client
        .put(&format!(
            "https://api.github.com/notifications/threads/{}/subscription",
            thread_id
        ))
        .json(&json!({"ignored": true}))
        .send()?;

    if resp.status() != 200 {
        return Err(ErrorKind::ResponseStatusError(resp.status()).into());
    }

    Ok(())
}

fn mark_a_thread_as_read(client: &Client, thread_id: &String) -> Fallible<()> {
    let resp = client
        .patch(&format!(
            "https://api.github.com/notifications/threads/{}",
            thread_id
        ))
        .send()?;

    if resp.status() != 205 {
        return Err(ErrorKind::ResponseStatusError(resp.status()).into());
    }

    Ok(())
}

fn compile_regex() -> Fallible<regex::Regex> {
    let path = dirs::home_dir().unwrap().join(".ghnf").join("filters");
    let f = std::fs::File::open(path)?;
    let filters = BufReader::new(f)
        .lines()
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let filters_string = String::from(r"(?i)") + &filters.join("|");
    match regex::Regex::new(&filters_string) {
        Ok(i) => return Ok(i),
        Err(i) => return Err(ErrorKind::RegexError(i).into()),
    }
}

fn create_client() -> Fallible<Client> {
    let path = dirs::home_dir().unwrap().join(".ghnf").join("token");
    let token = std::fs::read_to_string(path)?
        .split('\n')
        .nth(0)
        .unwrap()
        .to_owned();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("token {}", token))?,
    );
    let c = reqwest::Client::builder().default_headers(headers).build();
    match c {
        Ok(i) => return Ok(i),
        Err(i) => return Err(ErrorKind::ReqwestError(i).into()),
    }
}

fn main() {
    let m = clap::App::new("")
        .arg(
            clap::Arg::with_name("list")
                .long("list")
                .help("List all subscriptions"),
        )
        .get_matches();

    let list = m.is_present("list");

    let c = create_client().unwrap();
    let ss = get_notification_subscriptions(&c).unwrap();
    if list {
        for s in ss {
            println!("{} : {}", s.subject.r#type, s.subject.title);
        }
        return;
    }

    let re = compile_regex().unwrap();
    let candidates: Vec<_> = ss
        .into_iter()
        .filter(|s| re.is_match(&s.subject.title))
        .collect();
    if candidates.is_empty() {
        println!("No notification matched");
        return;
    }

    for s in candidates.iter() {
        println!("{} : {}", s.subject.r#type, s.subject.title);
    }

    println!("To unsubscribe the notification(s), press Enter\n(If you don't want to, just abort (e.g. Ctrl+C))");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);

    for s in candidates.iter() {
        unsubscribe_thread(&c, &s.thread_id).unwrap();
        mark_a_thread_as_read(&c, &s.thread_id).unwrap();
        println!(
            "Unsubscribed {} : {} ({})",
            s.subject.r#type, s.subject.title, s.subject.url
        );
    }
}
