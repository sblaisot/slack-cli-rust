pub mod slack;
pub mod token;

use crate::slack::{
    Attachment, AttachmentPayload, Block, BlocksPayload, HeaderBlock, SectionBlock, SlackClient,
    SlackResponse,
};
use std::fmt;

pub const ATTACHMENT_TEXT_MAX: usize = 4000;

pub struct SendConfig {
    pub channel: String,
    pub message: String,
    pub color: Option<String>,
    pub title: Option<String>,
    pub token: String,
}

pub struct SendResult {
    pub ok: bool,
    pub warning: Option<String>,
}

fn resolve_color(input: &str) -> Result<String, SlackCliError> {
    match input.to_lowercase().as_str() {
        "good" | "success" => Ok("#36a64f".to_string()),
        "warning" => Ok("#daa038".to_string()),
        "danger" | "error" => Ok("#a30200".to_string()),
        hex if hex.len() == 7
            && hex.starts_with('#')
            && hex[1..].chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            Ok(hex.to_string())
        }
        _ => Err(SlackCliError::InvalidColor(input.to_string())),
    }
}

pub fn send_message(
    client: &dyn SlackClient,
    config: &SendConfig,
) -> Result<SendResult, SlackCliError> {
    let resolved_color = config
        .color
        .as_ref()
        .map(|c| resolve_color(c))
        .transpose()?;

    let use_attachment = resolved_color.is_some() && config.message.len() <= ATTACHMENT_TEXT_MAX;

    let mut warning: Option<String> = None;

    if resolved_color.is_some() && config.message.len() > ATTACHMENT_TEXT_MAX {
        warning = Some(format!(
            "Message exceeds {} chars; sending without color",
            ATTACHMENT_TEXT_MAX
        ));
    }

    let mut blocks: Vec<Block> = Vec::new();
    if let Some(ref title) = config.title {
        blocks.push(Block::Header(HeaderBlock::new(title)));
    }
    blocks.push(Block::Section(SectionBlock::new(&config.message)));

    let payload_bytes = if use_attachment {
        let color = resolved_color.unwrap();
        let payload = AttachmentPayload {
            channel: config.channel.clone(),
            text: String::new(),
            attachments: vec![Attachment { color, blocks }],
        };
        serde_json::to_vec(&payload).unwrap()
    } else {
        let payload = BlocksPayload {
            channel: config.channel.clone(),
            text: config.message.clone(),
            blocks,
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
    InvalidColor(String),
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
            SlackCliError::InvalidColor(c) => write!(f, "invalid color '{c}': expected #RRGGBB or keyword (good, success, warning, danger, error)"),
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

    fn config(message: &str, color: Option<&str>, title: Option<&str>) -> SendConfig {
        SendConfig {
            channel: "#test".to_string(),
            message: message.to_string(),
            color: color.map(|c| c.to_string()),
            title: title.map(|t| t.to_string()),
            token: "xoxb-test".to_string(),
        }
    }

    #[test]
    fn test_no_color_sends_blocks_payload() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello world", None, None);
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
        let cfg = config("Hello", Some("#FF0000"), None);
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);
        assert!(result.warning.is_none());

        let json = client.captured_json();
        assert!(json.get("attachments").is_some());
        assert!(json.get("blocks").is_none());
        assert_eq!(json["text"], "");
        assert_eq!(json["attachments"][0]["color"], "#ff0000");
        assert_eq!(json["attachments"][0]["blocks"][0]["text"]["text"], "Hello");
    }

    #[test]
    fn test_color_long_message_falls_back_to_blocks() {
        let long_msg = "a".repeat(ATTACHMENT_TEXT_MAX + 1);
        let client = MockSlackClient::ok();
        let cfg = config(&long_msg, Some("#FF0000"), None);
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
        let cfg = config(&boundary_msg, Some("#00FF00"), None);
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);
        assert!(result.warning.is_none());

        let json = client.captured_json();
        assert!(json.get("attachments").is_some());
        assert_eq!(json["attachments"][0]["color"], "#00ff00");
    }

    #[test]
    fn test_color_one_over_boundary_falls_back() {
        let over_msg = "a".repeat(ATTACHMENT_TEXT_MAX + 1);
        let client = MockSlackClient::ok();
        let cfg = config(&over_msg, Some("#00FF00"), None);
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
        let cfg = config("Hello", None, None);
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
        let cfg = config("Hello", None, None);
        let result = send_message(&client, &cfg).unwrap();
        assert_eq!(result.warning.unwrap(), "missing_text_in_message");
    }

    #[test]
    fn test_resolve_color_valid_hex() {
        assert_eq!(resolve_color("#FF0000").unwrap(), "#ff0000");
    }

    #[test]
    fn test_resolve_color_good() {
        assert_eq!(resolve_color("good").unwrap(), "#36a64f");
    }

    #[test]
    fn test_resolve_color_success_case_insensitive() {
        assert_eq!(resolve_color("Success").unwrap(), "#36a64f");
    }

    #[test]
    fn test_resolve_color_warning() {
        assert_eq!(resolve_color("warning").unwrap(), "#daa038");
    }

    #[test]
    fn test_resolve_color_danger() {
        assert_eq!(resolve_color("danger").unwrap(), "#a30200");
    }

    #[test]
    fn test_resolve_color_error() {
        assert_eq!(resolve_color("error").unwrap(), "#a30200");
    }

    #[test]
    fn test_resolve_color_invalid_keyword() {
        assert!(matches!(
            resolve_color("blue"),
            Err(SlackCliError::InvalidColor(ref s)) if s == "blue"
        ));
    }

    #[test]
    fn test_resolve_color_invalid_hex_chars() {
        assert!(matches!(
            resolve_color("#GGG000"),
            Err(SlackCliError::InvalidColor(_))
        ));
    }

    #[test]
    fn test_resolve_color_invalid_hex_too_short() {
        assert!(matches!(
            resolve_color("#FFF"),
            Err(SlackCliError::InvalidColor(_))
        ));
    }

    #[test]
    fn test_title_without_color_sends_header_and_section_blocks() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello", None, Some("My Title"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);

        let json = client.captured_json();
        assert!(json.get("blocks").is_some());
        assert!(json.get("attachments").is_none());
        assert_eq!(json["blocks"][0]["type"], "header");
        assert_eq!(json["blocks"][0]["text"]["type"], "plain_text");
        assert_eq!(json["blocks"][0]["text"]["text"], "My Title");
        assert_eq!(json["blocks"][1]["type"], "section");
        assert_eq!(json["blocks"][1]["text"]["text"], "Hello");
    }

    #[test]
    fn test_title_with_color_sends_attachment_with_header_and_section() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello", Some("#FF0000"), Some("My Title"));
        let result = send_message(&client, &cfg).unwrap();
        assert!(result.ok);

        let json = client.captured_json();
        assert!(json.get("attachments").is_some());
        let blocks = &json["attachments"][0]["blocks"];
        assert_eq!(blocks[0]["type"], "header");
        assert_eq!(blocks[0]["text"]["text"], "My Title");
        assert_eq!(blocks[1]["type"], "section");
        assert_eq!(blocks[1]["text"]["text"], "Hello");
    }

    #[test]
    fn test_no_title_has_no_header_block() {
        let client = MockSlackClient::ok();
        let cfg = config("Hello", None, None);
        send_message(&client, &cfg).unwrap();

        let json = client.captured_json();
        assert_eq!(json["blocks"].as_array().unwrap().len(), 1);
        assert_eq!(json["blocks"][0]["type"], "section");
    }
}
