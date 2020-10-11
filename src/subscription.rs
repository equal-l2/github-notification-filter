use failure::{err_msg, format_err, Fallible};
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
    pub async fn from_thread_id(id: ThreadID, c: &Client) -> Fallible<Self> {
        let url = format!("https://api.github.com/notifications/threads/{}", id);
        let resp = c.get(&url).send().await?;

        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .await
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        serde_json::from_str::<Notification>(&resp.text().await?)
            .map_err(Into::into)
            .map(Into::into)
    }

    pub async fn open(&self, c: &Client) -> Fallible<()> {
        open::that(self.html_url(c).await?)
            .map(|_| ()) // discard ExitStatus
            .map_err(Into::into)
    }

    // TODO: rewrite with Stream
    pub async fn fetch_unread(client: &Client) -> Fallible<Vec<Vec<Self>>> {
        let url = "https://api.github.com/notifications";
        let head = client.head(url).send().await?;
        let last_page = crate::util::get_last_page(head.headers()["Link"].to_str().unwrap());

        let mut futs = vec![];

        for i in 1..=last_page {
            let i_str = i.to_string();
            futs.push(async move {
                let resp = client
                    .get("https://api.github.com/notifications")
                    .query(&[("page", &i_str)])
                    .send()
                    .await?;
                if resp.status() != 200 {
                    return Err(format_unexpected_status(
                        StatusCode::from_u16(200).unwrap(),
                        resp.status(),
                        url,
                        resp.text()
                            .await
                            .unwrap_or_else(|_| String::from("<Failed to get body>")),
                    ));
                }
                Ok(
                    serde_json::from_str::<Vec<Notification>>(&resp.text().await?)?
                        .into_iter()
                        .map(Into::into),
                )
            });
        }

        Ok(futures::future::try_join_all(futs)
            .await?
            .into_iter()
            .map(Iterator::collect)
            .collect())
    }

    pub async fn unsubscribe(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}/subscription",
            self.thread_id
        );
        let resp = client.delete(&url).send().await?;

        if resp.status() != 204 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(204).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .await
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    pub async fn mark_as_read(&self, client: &Client) -> Fallible<()> {
        let url = format!(
            "https://api.github.com/notifications/threads/{}",
            self.thread_id
        );
        let resp = client.patch(&url).send().await?;

        if resp.status() != 205 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(205).unwrap(),
                resp.status(),
                &url,
                resp.text()
                    .await
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }

        Ok(())
    }

    /// get url for subject's html location
    pub async fn html_url(&self, c: &Client) -> Fallible<String> {
        Ok(self.subject_detail(c).await?.html_url.to_owned())
    }

    /// get subject state (i.e. open or closed)
    pub async fn subject_state(&self, c: &Client) -> Fallible<Option<gh_objects::SubjectState>> {
        Ok(self.subject_detail(c).await?.state)
    }

    async fn fetch_subject_detail(&self, c: &Client) -> Fallible<SubjectDetail> {
        let url = &self.subject.url;
        let resp = c.get(url).send().await?;
        if resp.status() != 200 {
            return Err(format_unexpected_status(
                StatusCode::from_u16(200).unwrap(),
                resp.status(),
                url,
                resp.text()
                    .await
                    .unwrap_or_else(|_| String::from("<Failed to get body>")),
            ));
        }
        serde_json::from_str(&resp.text().await?).map_err(Into::into)
    }

    async fn subject_detail(&self, c: &Client) -> Fallible<&SubjectDetail> {
        if self.subject_detail.get().is_none() {
            let res = self.fetch_subject_detail(c).await?;
            self.subject_detail
                .set(res)
                .expect("subject_detail is re-initialized");
        }
        Ok(self.subject_detail.get().unwrap())
    }
}
