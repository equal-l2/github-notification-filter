#[derive(Debug, failure::Fail)]
pub enum ErrorKind {
    #[fail(display = "Unexpected response status : {}", _0)]
    ResponseStatusError(reqwest::StatusCode),
    #[fail(display = "Regex error : {}", _0)]
    RegexError(regex::Error),
    #[fail(display = "Reqwest error : {}", _0)]
    ReqwestError(reqwest::Error),
}
