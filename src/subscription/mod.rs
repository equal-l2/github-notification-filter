use failure::Fallible;
use reqwest::Client;
use serde_json::json;

mod gh_objects;
use crate::ErrorKind;
use gh_objects::Notification;

#[derive(Clone, Debug)]
pub struct Subscription {
    pub subject: gh_objects::Subject,
    thread_id: String,
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.url.split('/').last().unwrap().into(),
        }
    }
}

impl std::fmt::Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} : {}", self.subject.r#type, self.subject.title)
    }
}

impl Subscription {
    pub fn fetch_unread(client: &Client) -> Fallible<Vec<Subscription>> {
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

    pub fn unsubscribe_thread(&self, client: &Client) -> Fallible<()> {
        let resp = client
            .put(&format!(
                "https://api.github.com/notifications/threads/{}/subscription",
                self.thread_id
            ))
            .json(&json!({"ignored": true}))
            .send()?;

        if resp.status() != 200 {
            return Err(ErrorKind::ResponseStatusError(resp.status()).into());
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
            return Err(ErrorKind::ResponseStatusError(resp.status()).into());
        }

        Ok(())
    }
}
