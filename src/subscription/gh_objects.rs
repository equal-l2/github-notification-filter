use failure::{err_msg, format_err, Fallible};
use reqwest::Client;
use serde_json::Value;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Notification {
    pub subject: Subject,
    pub url: String,
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
pub struct Subject {
    pub title: String,
    pub url: String,
    pub r#type: SubjectType,
    // not currently used
    //latest_comment_url: Option<String>,
}

impl Subject {
    pub fn get_html_url(&self, client: &Client) -> Fallible<String> {
        let mut resp = client.get(&self.url).send()?;

        if resp.status() != 200 {
            Err(err_msg(format_err!(
                "Unexpected HTTP Status {} (Expected 200)",
                resp.status()
            )))?
        }

        Ok(resp.json::<Value>()?["html_url"]
            .as_str()
            .unwrap()
            .to_owned())
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(field_identifier)]
pub enum SubjectType {
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
