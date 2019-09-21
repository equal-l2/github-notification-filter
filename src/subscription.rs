use failure::{err_msg, format_err, Fallible};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::sync::RwLock;

mod gh_objects;
use gh_objects::Notification;
pub use gh_objects::SubjectState;

pub type ThreadID = u64;

#[derive(Debug)]
pub struct Subscription {
    pub subject: gh_objects::Subject,
    pub thread_id: ThreadID,
    pub repo_name: String,
    subject_detail: RwLock<Option<gh_objects::SubjectDetail>>,
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.url.split('/').last().unwrap().parse().unwrap(),
            repo_name: n.repository.full_name,
            subject_detail: RwLock::new(None),
        }
    }
}

impl std::fmt::Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} : {} ({})",
            self.subject.r#type, self.repo_name, self.subject.title, self.thread_id
        )
    }
}

fn format_unexpected_status(
    expected: StatusCode,
    actual: StatusCode,
    url: &str,
    body: String,
) -> failure::Error {
    err_msg(format_err!(
        "Unexpected HTTP Status {} (Expected {})\nURL: {}\nBody: {}",
        actual,
        expected,
        url,
        body
    ))
}

impl Subscription {
    pub fn from_thread_id(id: ThreadID, c: &Client) -> Fallible<Subscription> {
        let url = format!("https://api.github.com/notifications/threads/{}", id);
        let mut resp = c.get(&url).send()?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                &url,
                resp.text().unwrap_or(String::from("<Failed to get body>")),
            ));
        }

        resp.json::<Notification>()
            .map_err(Into::into)
            .map(Into::into)
    }

    pub fn open_thread(&self, c: &Client) -> Fallible<()> {
        open::that(self.get_html_url(&c)?)
            .map(|_| ()) // discard ExitStatus
            .map_err(Into::into)
    }

    pub fn fetch_unread(client: &Client) -> Fallible<Vec<Subscription>> {
        let url = "https://api.github.com/notifications";
        let mut resp = client.get(url).send()?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                &url,
                resp.text().unwrap_or(String::from("<Failed to get body>")),
            ));
        }

        let mut ss: Vec<_> = {
            let ns: Vec<Notification> = resp.json()?;
            ns.into_iter().map(Into::into).collect()
        };

        for i in 2.. {
            let mut resp = client
                .get("https://api.github.com/notifications")
                .query(&[("page", &i.to_string())])
                .send()?;
            if resp.status() == 200 {
                let ns: Vec<Notification> = resp.json()?;
                if !ns.is_empty() {
                    ss.extend(ns.into_iter().map(Into::into));
                    continue;
                }
            }
            return Ok(ss);
        }
        unreachable!();
    }

    pub fn unsubscribe_thread(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}/subscription",
            self.thread_id
        );
        let mut resp = client.put(&url).json(&json!({"ignored": true})).send()?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                &url,
                resp.text().unwrap_or(String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    pub fn mark_a_thread_as_read(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}",
            self.thread_id
        );
        let mut resp = client.patch(&url).send()?;

        if resp.status() != 205 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(205).unwrap(),
                resp.status(),
                &url,
                resp.text().unwrap_or(String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    pub fn get_html_url(&self, c: &Client) -> Fallible<String> {
        // self.subject_detail.read() cannot be in a variable
        // because it prevents anyone from writing to subject_detail while it lives
        if self.subject_detail.read().unwrap().is_none() {
            self.fetch_subject_detail(c)?;
        }
        Ok(self
            .subject_detail
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .html_url
            .to_owned())
    }

    pub fn get_subject_state(&self, c: &Client) -> Fallible<Option<gh_objects::SubjectState>> {
        // self.subject_detail.read() cannot be in a variable
        // because it prevents anyone from writing to subject_detail while it lives
        if self.subject_detail.read().unwrap().is_none() {
            self.fetch_subject_detail(c)?;
        }
        Ok(self.subject_detail.read().unwrap().as_ref().unwrap().state)
    }

    fn fetch_subject_detail(&self, c: &Client) -> Fallible<()> {
        let url = &self.subject.url;
        let mut resp = c.get(url).send()?;
        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                url,
                resp.text().unwrap_or(String::from("<Failed to get body>")),
            ));
        }
        let result: gh_objects::SubjectDetail = resp.json()?;
        *self.subject_detail.write().unwrap() = Some(result);
        Ok(())
    }
}
