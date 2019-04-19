use failure::{err_msg, format_err, Fallible};
use reqwest::Client;
use serde_json::json;

mod gh_objects;
use gh_objects::Notification;

pub type ThreadID = u64;

#[derive(Clone, Debug)]
pub struct Subscription {
    pub subject: gh_objects::Subject,
    pub thread_id: ThreadID,
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.url.split('/').last().unwrap().parse().unwrap(),
        }
    }
}

impl std::fmt::Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} : {} ({})",
            self.subject.r#type, self.subject.title, self.thread_id
        )
    }
}

impl Subscription {
    pub fn from_thread_id(id: ThreadID, c: &Client) -> Fallible<Subscription> {
        let mut resp = c
            .get(&format!(
                "https://api.github.com/notifications/threads/{}",
                id
            ))
            .send()?;

        if resp.status() != 200 {
            Err(err_msg(format_err!(
                "Unexpected HTTP Status {} (Expected 200)",
                resp.status()
            )))?
        }

        Ok(resp.json::<Notification>()?.into())
    }

    pub fn open_thread(&self, c: &Client) -> Fallible<()> {
        open::that(self.subject.get_html_url(&c)?)
            .map(|_| ()) // discard ExitStatus
            .map_err(|e| failure::Error::from(e))
    }

    pub fn fetch_unread(client: &Client) -> Fallible<Vec<Subscription>> {
        let mut resp = client.get("https://api.github.com/notifications").send()?;

        if resp.status() != 200 {
            Err(err_msg(format_err!(
                "Unexpected HTTP Status {} (Expected 200)",
                resp.status()
            )))?
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

    pub fn unsubscribe_thread(&self, client: &Client) -> Fallible<()> {
        let resp = client
            .put(&format!(
                "https://api.github.com/notifications/threads/{}/subscription",
                self.thread_id
            ))
            .json(&json!({"ignored": true}))
            .send()?;

        if resp.status() != 200 {
            Err(err_msg(format_err!(
                "Unexpected HTTP Status {} (Expected 200)",
                resp.status()
            )))?
        }

        Ok(())
    }

    pub fn mark_a_thread_as_read(&self, client: &Client) -> Fallible<()> {
        let resp = client
            .patch(&format!(
                "https://api.github.com/notifications/threads/{}",
                self.thread_id
            ))
            .send()?;

        if resp.status() != 205 {
            Err(err_msg(format_err!(
                "Unexpected HTTP Status {} (Expected 205)",
                resp.status()
            )))?
        }

        Ok(())
    }
}
