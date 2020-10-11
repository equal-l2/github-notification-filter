#[derive(Clone, Debug, serde::Deserialize)]
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Issue => write!(f, "Issue"),
            Self::PullRequest => write!(f, "Pull Request"),
            Self::Commit => write!(f, "Commit"),
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

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Repository {
    pub full_name: String,
}
