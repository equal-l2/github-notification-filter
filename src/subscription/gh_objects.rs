use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Notification {
    pub id: String,
    pub repository: Repository,
    pub subject: Subject,
    pub updated_at: String,
    /* fields not currently used:
    pub url: String,
    pub last_read_at: Option<String>,
    pub reason: String,
    pub subscription_url: String,
    pub unread: bool,
    */
}

#[derive(Clone, Debug, Deserialize)]
pub struct Subject {
    pub title: String,
    pub url: Option<String>, // not exists for discussions (This must be a FIXME, GitHub!)
    pub r#type: SubjectType,
    // not currently used
    //latest_comment_url: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(field_identifier)]
pub enum SubjectType {
    Commit,
    Discussion,
    Issue,
    PullRequest,
}

impl std::fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit => write!(f, "Commit"),
            Self::Discussion => write!(f, "Discuss"),
            Self::Issue => write!(f, "Issue"),
            Self::PullRequest => write!(f, "PullReq"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectState {
    Open,
    Closed,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SubjectDetail {
    pub url: String,
    pub html_url: String,
    pub state: Option<SubjectState>, // doesn't exist for commits
    pub title: Option<String>,       // doesn't exist for commits
}

#[derive(Clone, Debug, Deserialize)]
pub struct Repository {
    pub full_name: String,
}
