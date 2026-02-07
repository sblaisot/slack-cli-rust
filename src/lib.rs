pub mod slack;
pub mod token;

use crate::slack::{
    Attachment, AttachmentPayload, BlocksPayload, SectionBlock, SlackClient, SlackResponse,
};
use std::fmt;

pub const ATTACHMENT_TEXT_MAX: usize = 4000;

pub struct SendConfig {
    pub channel: String,
    pub message: String,
    pub color: Option<String>,
    pub token: String,
}

pub struct SendResult {
    pub ok: bool,
    pub warning: Option<String>,
}

pub fn send_message(
    client: &dyn SlackClient,
    config: &SendConfig,
) -> Result<SendResult, SlackCliError> {
    let use_attachment = config.color.is_some() && config.message.len() <= ATTACHMENT_TEXT_MAX;

    let mut warning: Option<String> = None;

    if config.color.is_some() && config.message.len() > ATTACHMENT_TEXT_MAX {
        warning = Some(format!(
            "Message exceeds {} chars; sending without color",
            ATTACHMENT_TEXT_MAX
        ));
    }

    let payload_bytes = if use_attachment {
        let color = config.color.as_ref().unwrap();
        let payload = AttachmentPayload {
            channel: config.channel.clone(),
            text: config.message.clone(),
            attachments: vec![Attachment {
                color: color.clone(),
                blocks: vec![SectionBlock::new(&config.message)],
            }],
        };
        serde_json::to_vec(&payload).unwrap()
    } else {
        let payload = BlocksPayload {
            channel: config.channel.clone(),
            text: config.message.clone(),
            blocks: vec![SectionBlock::new(&config.message)],
        };
        serde_json::to_vec(&payload).unwrap()
    };

    let response: SlackResponse = client.post_message(&config.token, &payload_bytes)?;

    if !response.ok {
        let error_msg = response
            .error
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(SlackCliError::SlackApiError(error_msg));
    }

    if warning.is_none() {
        warning = response.warning;
    }

    Ok(SendResult { ok: true, warning })
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockSlackClient {
        captured_payload: RefCell<Vec<u8>>,
        response: SlackResponse,
    }

    impl MockSlackClient {
        fn new(response: SlackResponse) -> Self {
            MockSlackClient {
                captured_payload: RefCell::new(Vec::new()),
                response,
            }
        }

        fn ok() -> Self {
            MockSlackClient::new(SlackResponse {
                ok: true,
                error: None,
                warning: None,
            })
        }

        fn captured_json(&self) -> serde_json::Value {
            serde_json::from_slice(&self.captured_payload.borrow()).unwrap()
        }
    }

    impl SlackClient for MockSlackClient {
        fn post_message(
            &self,
            _token: &str,
            payload: &[u8],
        ) -> Result<SlackResponse, SlackCliError> {
            *self.captured_payload.borrow_mut() = payload.to_vec();
            Ok(SlackResponse {
                ok: self.response.ok,
                error: self.response.error.clone(),
                warning: self.response.warning.clone(),
            })
        }
    }

    fn config(message: &str, color: Option<&str>) -> SendConfig {
        SendConfig {
            channel: "#test".to_string(),
            message: message.to_string(),
            color: color.map(|c| c.to_string()),
            token: "xoxb-test".to_string(),
        }
    }

    #[test]
    fn test_no_color_sends_blocks_payload() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello world", None);
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);

        let json = client.captured_json();
        assert_eq!(json["channel"], "#test");
        assert_eq!(json["text"], "Hello world");
        assert!(json.get("blocks").is_some());
        assert!(json.get("attachments").is_none());
        assert_eq!(json["blocks"][0]["type"], "section");
        assert_eq!(json["blocks"][0]["text"]["text"], "Hello world");
    }

    #[test]
    fn test_color_short_message_sends_attachment() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello", Some("#FF0000"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);
        assert!(result.warning.is_none());

        let json = client.captured_json();
        assert!(json.get("attachments").is_some());
        assert!(json.get("blocks").is_none());
        assert_eq!(json["attachments"][0]["color"], "#FF0000");
        assert_eq!(json["attachments"][0]["blocks"][0]["text"]["text"], "Hello");
    }

    #[test]
    fn test_color_long_message_falls_back_to_blocks() {
        let long_msg = "a".repeat(ATTACHMENT_TEXT_MAX + 1);
        let client = MockSlackClient::ok();
        let cfg = config(&long_msg, Some("#FF0000"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("4000"));

        let json = client.captured_json();
        assert!(json.get("blocks").is_some());
        assert!(json.get("attachments").is_none());
    }

    #[test]
    fn test_color_at_boundary_sends_attachment() {
        let boundary_msg = "a".repeat(ATTACHMENT_TEXT_MAX);
        let client = MockSlackClient::ok();
        let cfg = config(&boundary_msg, Some("#00FF00"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);
        assert!(result.warning.is_none());

        let json = client.captured_json();
        assert!(json.get("attachments").is_some());
        assert_eq!(json["attachments"][0]["color"], "#00FF00");
    }

    #[test]
    fn test_color_one_over_boundary_falls_back() {
        let over_msg = "a".repeat(ATTACHMENT_TEXT_MAX + 1);
        let client = MockSlackClient::ok();
        let cfg = config(&over_msg, Some("#00FF00"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.warning.is_some());

        let json = client.captured_json();
        assert!(json.get("blocks").is_some());
        assert!(json.get("attachments").is_none());
    }

    #[test]
    fn test_api_error_returns_error() {
        let client = MockSlackClient::new(SlackResponse {
            ok: false,
            error: Some("channel_not_found".to_string()),
            warning: None,
        });
        let cfg = config("Hello", None);
        let result = send_message(&client, &cfg);
        assert!(
            matches!(result, Err(SlackCliError::SlackApiError(ref e)) if e == "channel_not_found")
        );
    }

    #[test]
    fn test_api_warning_passed_through() {
        let client = MockSlackClient::new(SlackResponse {
            ok: true,
            error: None,
            warning: Some("missing_text_in_message".to_string()),
        });
        let cfg = config("Hello", None);
        let result = send_message(&client, &cfg).unwrap();
        assert_eq!(result.warning.unwrap(), "missing_text_in_message");
    }
}
