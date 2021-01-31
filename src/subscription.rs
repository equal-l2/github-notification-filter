use anyhow::{anyhow, Result};
use once_cell::unsync::OnceCell;
use reqwest::Client;
use reqwest::StatusCode;

pub mod gh_objects;
use gh_objects::Notification;
pub use gh_objects::SubjectDetail;
pub use gh_objects::SubjectState;

pub type ThreadID = u64;

#[derive(Clone, Debug)]
pub struct Subscription {
    pub subject: gh_objects::Subject,
    pub thread_id: ThreadID,
    pub repo_name: String,
    pub updated_at: String, //TODO: use correct type
    subject_detail: OnceCell<SubjectDetail>,
}

impl From<Notification> for Subscription {
    fn from(n: Notification) -> Self {
        Self {
            subject: n.subject,
            thread_id: n.id.parse().unwrap(),
            repo_name: n.repository.full_name,
            subject_detail: OnceCell::new(),
            updated_at: n.updated_at,
        }
    }
}

impl std::fmt::Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} : {} ({}) at {}",
            self.subject.r#type,
            self.repo_name,
            self.subject.title,
            self.thread_id,
            self.updated_at
        )
    }
}

#[derive(Debug, thiserror::Error)]
enum StatusError {
    #[error("Rate limit handled")]
    RateLimit,
    #[error("Unexpected status")]
    Unexpected(String),
}

async fn check_unexpected_status(expected: u16, resp: reqwest::Response) -> Result<String> {
    use tokio::time;
    let now = time::Instant::now();
    if resp.status() != expected {
        return Err(if let Some(t) = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
        {
            eprintln!("Rate limit exceeded, wait for {} secs", t);
            let _ = resp.text().await?;
            time::sleep_until(now + time::Duration::from_secs(t)).await;
            StatusError::RateLimit
        } else {
            let errmsg = fmt_unexpected_status(expected, resp).await;
            StatusError::Unexpected(errmsg)
        }
        .into());
    } else {
        let text = resp.text().await?;
        return Ok(text);
    }
}

async fn fmt_unexpected_status(expected: u16, resp: reqwest::Response) -> String {
    use std::fmt::Write;

    let mut ret = format!(
        "Unexpected HTTP Status {} (Expected {})\nURL: {}",
        resp.status(),
        StatusCode::from_u16(expected).unwrap(),
        resp.url(),
    );

    {
        write!(ret, "\nHeaders:").unwrap();

        let headers = resp.headers();
        for (k, v) in headers {
            let v = v.to_str().unwrap_or("<Not representable in string>");
            write!(ret, "\n{} : {}", k, v).unwrap();
        }
    }

    write!(
        ret,
        "\nBody: {}",
        resp.text()
            .await
            .unwrap_or_else(|_| String::from("<Failed to get body>"))
    )
    .unwrap();

    ret
}

impl Subscription {
    pub async fn from_thread_id(id: ThreadID, c: &Client) -> Result<Self> {
        let url = format!("https://api.github.com/notifications/threads/{}", id);
        loop {
            let resp = c.get(&url).send().await?;
            match check_unexpected_status(200, resp).await {
                Ok(s) => {
                    return serde_json::from_str::<Notification>(&s)
                        .map_err(Into::into)
                        .map(Into::into)
                }
                Err(e) => match e.downcast() {
                    Ok(StatusError::RateLimit) => { /* retrying */ }
                    Ok(StatusError::Unexpected(s)) => return Err(anyhow!(s)),
                    Err(e) => return Err(e),
                },
            }
        }
    }

    pub async fn open(&self, c: &Client) -> Result<()> {
        open::that(self.html_url(c).await?)
            .map(|_| ()) // discard ExitStatus
            .map_err(Into::into)
    }

    // TODO: rewrite with Stream
    pub async fn fetch_unread(client: &Client) -> Result<Vec<Vec<Self>>> {
        let url = "https://api.github.com/notifications";
        let head = client.head(url).send().await?;
        let last_page = crate::util::get_last_page(head.headers()["Link"].to_str().unwrap());

        let mut futs = vec![];

        for i in 1..=last_page {
            let i_str = i.to_string();
            futs.push(async move {
                loop {
                    let resp = client
                        .get("https://api.github.com/notifications")
                        .query(&[("page", &i_str)])
                        .send()
                        .await?;

                    match check_unexpected_status(200, resp).await {
                        Ok(s) => {
                            return Ok(serde_json::from_str::<Vec<Notification>>(&s)?
                                .into_iter()
                                .map(Into::into));
                        }
                        Err(e) => match e.downcast() {
                            Ok(StatusError::RateLimit) => { /* retrying */ }
                            Ok(StatusError::Unexpected(s)) => return Err(anyhow!(s)),
                            Err(e) => return Err(e),
                        },
                    }
                }
            });
        }

        Ok(futures::future::try_join_all(futs)
            .await?
            .into_iter()
            .map(Iterator::collect)
            .collect())
    }

    pub async fn unsubscribe(&self, c: &Client) -> Result<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}/subscription",
            self.thread_id
        );

        loop {
            let resp = c.delete(&url).send().await?;
            match check_unexpected_status(204, resp).await {
                Ok(_) => return Ok(()),
                Err(e) => match e.downcast() {
                    Ok(StatusError::RateLimit) => { /* retrying */ }
                    Ok(StatusError::Unexpected(s)) => return Err(anyhow!(s)),
                    Err(e) => return Err(e),
                },
            }
        }
    }

    pub async fn mark_as_read(&self, c: &Client) -> Result<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}",
            self.thread_id
        );

        loop {
            let resp = c.patch(&url).send().await?;
            match check_unexpected_status(205, resp).await {
                Ok(_) => return Ok(()),
                Err(e) => match e.downcast() {
                    Ok(StatusError::RateLimit) => { /* retrying */ }
                    Ok(StatusError::Unexpected(s)) => return Err(anyhow!(s)),
                    Err(e) => return Err(e),
                },
            }
        }
    }

    /// get url for subject's html location
    pub async fn html_url(&self, c: &Client) -> Result<String> {
        Ok(self.subject_detail(c).await?.html_url.to_owned())
    }

    /// get subject state (i.e. open or closed)
    pub async fn subject_state(&self, c: &Client) -> Result<Option<gh_objects::SubjectState>> {
        Ok(self.subject_detail(c).await?.state)
    }

    async fn fetch_subject_detail(&self, c: &Client) -> Result<SubjectDetail> {
        let url = &self
            .subject
            .url
            .as_ref()
            .expect("tried to fetch subject_detail of Discussions");

        loop {
            let resp = c.get(*url).send().await?;
            match check_unexpected_status(200, resp).await {
                Ok(s) => return serde_json::from_str(&s).map_err(Into::into),
                Err(e) => match e.downcast() {
                    Ok(StatusError::RateLimit) => { /* retrying */ }
                    Ok(StatusError::Unexpected(s)) => return Err(anyhow!(s)),
                    Err(e) => return Err(e),
                },
            }
        }
    }

    async fn subject_detail(&self, c: &Client) -> Result<&SubjectDetail> {
        if self.subject_detail.get().is_none() {
            let res = self.fetch_subject_detail(c).await?;
            self.subject_detail
                .set(res)
                .expect("subject_detail is re-initialized");
        }
        Ok(self.subject_detail.get().unwrap())
    }
}
