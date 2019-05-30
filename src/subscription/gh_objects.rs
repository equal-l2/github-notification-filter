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

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(field_identifier)]
pub enum SubjectType {
    Issue,
    PullRequest,
    Commit,
}

impl std::fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SubjectType::Issue => write!(f, "Issue"),
            SubjectType::PullRequest => write!(f, "Pull Request"),
            SubjectType::Commit => write!(f, "Commit"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectState {
    Open,
    Closed,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SubjectDetail {
    pub url: String,
    pub html_url: String,
    pub state: Option<SubjectState>, // doesn't exist for commits
    pub title: Option<String>,       // doesn't exist for commits
}
