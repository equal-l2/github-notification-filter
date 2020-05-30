use failure::{err_msg, format_err, Fallible};
use once_cell::unsync::OnceCell;
use reqwest::blocking::Client;
use reqwest::StatusCode;

pub mod gh_objects;
use gh_objects::Notification;
pub use gh_objects::SubjectState;

pub type ThreadID = u64;

#[derive(Debug)]
pub struct Subscription {
    pub subject: gh_objects::Subject,
    pub thread_id: ThreadID,
    pub repo_name: String,
    subject_detail: OnceCell<gh_objects::SubjectDetail>,
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.url.split('/').last().unwrap().parse().unwrap(),
            repo_name: n.repository.full_name,
            subject_detail: OnceCell::new(),
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
    pub fn from_thread_id(id: ThreadID, c: &Client) -> Fallible<Self> {
        let url = format!("https://api.github.com/notifications/threads/{}", id);
        let resp = c.get(&url).send()?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        resp.json::<Notification>()
            .map_err(Into::into)
            .map(Into::into)
    }

    pub fn open(&self, c: &Client) -> Fallible<()> {
        open::that(self.get_html_url(c)?)
            .map(|_| ()) // discard ExitStatus
            .map_err(Into::into)
    }

    pub fn fetch_unread(client: &Client) -> Fallible<Vec<Self>> {
        let url = "https://api.github.com/notifications";
        let resp = client.get(url).send()?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                url,
                resp.text()
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        let mut ss = resp
            .json::<Vec<Notification>>()?
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();

        for i in 2.. {
            let resp = client
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

    pub fn unsubscribe(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}/subscription",
            self.thread_id
        );
        let resp = client.delete(&url).send()?;

        if resp.status() != 204 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(204).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    pub fn mark_as_read(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}",
            self.thread_id
        );
        let resp = client.patch(&url).send()?;

        if resp.status() != 205 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(205).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    /// get url for subject's html location
    pub fn get_html_url(&self, c: &Client) -> Fallible<String> {
        if self.subject_detail.get().is_none() {
            self.fetch_subject_detail(c)?;
        }
        Ok(self.subject_detail.get().unwrap().html_url.to_owned())
    }

    /// get subject state (i.e. open or closed)
    pub fn get_subject_state(&self, c: &Client) -> Fallible<Option<gh_objects::SubjectState>> {
        if self.subject_detail.get().is_none() {
            self.fetch_subject_detail(c)?;
        }
        Ok(self.subject_detail.get().unwrap().state)
    }

    fn fetch_subject_detail(&self, c: &Client) -> Fallible<()> {
        let url = &self.subject.url;
        let resp = c.get(url).send()?;
        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                url,
                resp.text()
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }
        let result: gh_objects::SubjectDetail = resp.json()?;
        self.subject_detail
            .set(result)
            .expect("subject_detail is re-initialized");
        Ok(())
    }
}
