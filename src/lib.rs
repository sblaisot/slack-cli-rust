pub mod slack;
pub mod token;

use std::fmt;

pub const ATTACHMENT_TEXT_MAX: usize = 4000;

#[derive(Debug)]
pub enum SlackCliError {
    TokenNotFound,
    TokenReadError(std::io::Error),
    HttpError(reqwest::Error),
    SlackApiError(String),
    NoMessage,
    StdinError(std::io::Error),
}

impl fmt::Display for SlackCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlackCliError::TokenNotFound => write!(
                f,
                "Slack API token not found. Set SLACK_API_KEY env var, or place token in ~/.slack/api-token or /etc/slack/api-token"
            ),
            SlackCliError::TokenReadError(e) => write!(f, "Failed to read token file: {e}"),
            SlackCliError::HttpError(e) => write!(f, "HTTP request failed: {e}"),
            SlackCliError::SlackApiError(e) => write!(f, "Slack API error: {e}"),
            SlackCliError::NoMessage => write!(f, "No message provided"),
            SlackCliError::StdinError(e) => write!(f, "Failed to read stdin: {e}"),
        }
    }
}

impl std::error::Error for SlackCliError {}

impl From<reqwest::Error> for SlackCliError {
    fn from(err: reqwest::Error) -> Self {
        SlackCliError::HttpError(err)
    }
}
