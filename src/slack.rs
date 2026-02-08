use crate::SlackCliError;
use serde::{Deserialize, Serialize};

const SLACK_API_URL: &str = "https://slack.com/api/chat.postMessage";

#[derive(Serialize)]
pub struct TextObject {
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct SectionBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: TextObject,
}

impl SectionBlock {
    pub fn new(text: &str) -> Self {
        SectionBlock {
            block_type: "section".to_string(),
            text: TextObject {
                text_type: "mrkdwn".to_string(),
                text: text.to_string(),
            },
        }
    }
}

#[derive(Serialize)]
pub struct HeaderBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: TextObject,
}

impl HeaderBlock {
    pub fn new(text: &str) -> Self {
        HeaderBlock {
            block_type: "header".to_string(),
            text: TextObject {
                text_type: "plain_text".to_string(),
                text: text.to_string(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Block {
    Header(HeaderBlock),
    Section(SectionBlock),
}

#[derive(Serialize)]
pub struct BlocksPayload {
    pub channel: String,
    pub text: String,
    pub blocks: Vec<Block>,
}

#[derive(Serialize)]
pub struct Attachment {
    pub color: String,
    pub blocks: Vec<Block>,
}

#[derive(Serialize)]
pub struct AttachmentPayload {
    pub channel: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Deserialize, Debug)]
pub struct SlackResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub warning: Option<String>,
}

pub trait SlackClient {
    fn post_message(&self, token: &str, payload: &[u8]) -> Result<SlackResponse, SlackCliError>;
}

pub struct HttpSlackClient;

impl SlackClient for HttpSlackClient {
    fn post_message(&self, token: &str, payload: &[u8]) -> Result<SlackResponse, SlackCliError> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(SLACK_API_URL)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json; charset=utf-8")
            .body(payload.to_vec())
            .send()?;

        let slack_response: SlackResponse = response.json()?;
        Ok(slack_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_payload_serialization() {
        let payload = BlocksPayload {
            channel: "#general".to_string(),
            text: "Hello world".to_string(),
            blocks: vec![Block::Section(SectionBlock::new("Hello world"))],
        };
        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["channel"], "#general");
        assert_eq!(json["text"], "Hello world");
        assert_eq!(json["blocks"][0]["type"], "section");
        assert_eq!(json["blocks"][0]["text"]["type"], "mrkdwn");
        assert_eq!(json["blocks"][0]["text"]["text"], "Hello world");
    }

    #[test]
    fn test_attachment_payload_serialization() {
        let payload = AttachmentPayload {
            channel: "#general".to_string(),
            text: "Hello world".to_string(),
            attachments: vec![Attachment {
                color: "#FF0000".to_string(),
                blocks: vec![Block::Section(SectionBlock::new("Hello world"))],
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["channel"], "#general");
        assert_eq!(json["text"], "Hello world");
        assert_eq!(json["attachments"][0]["color"], "#FF0000");
        assert_eq!(json["attachments"][0]["blocks"][0]["type"], "section");
        assert_eq!(
            json["attachments"][0]["blocks"][0]["text"]["text"],
            "Hello world"
        );
    }

    #[test]
    fn test_header_block_serialization() {
        let block = Block::Header(HeaderBlock::new("My Title"));
        let json: serde_json::Value = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "header");
        assert_eq!(json["text"]["type"], "plain_text");
        assert_eq!(json["text"]["text"], "My Title");
    }

    #[test]
    fn test_json_escaping_special_chars() {
        let payload = BlocksPayload {
            channel: "#general".to_string(),
            text: "Line1\nLine2\t\"quoted\" and \\backslash".to_string(),
            blocks: vec![Block::Section(SectionBlock::new(
                "Line1\nLine2\t\"quoted\" and \\backslash",
            ))],
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["text"], "Line1\nLine2\t\"quoted\" and \\backslash");
    }

    #[test]
    fn test_json_escaping_unicode() {
        let payload = BlocksPayload {
            channel: "#general".to_string(),
            text: "Hello 🌍 world".to_string(),
            blocks: vec![Block::Section(SectionBlock::new("Hello 🌍 world"))],
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["text"], "Hello 🌍 world");
    }

    #[test]
    fn test_slack_response_ok() {
        let json = r#"{"ok": true}"#;
        let response: SlackResponse = serde_json::from_str(json).unwrap();
        assert!(response.ok);
        assert!(response.error.is_none());
        assert!(response.warning.is_none());
    }

    #[test]
    fn test_slack_response_error() {
        let json = r#"{"ok": false, "error": "channel_not_found"}"#;
        let response: SlackResponse = serde_json::from_str(json).unwrap();
        assert!(!response.ok);
        assert_eq!(response.error.unwrap(), "channel_not_found");
    }

    #[test]
    fn test_slack_response_warning() {
        let json = r#"{"ok": true, "warning": "missing_text_in_message"}"#;
        let response: SlackResponse = serde_json::from_str(json).unwrap();
        assert!(response.ok);
        assert_eq!(response.warning.unwrap(), "missing_text_in_message");
    }

    #[test]
    fn test_no_attachments_key_in_blocks_payload() {
        let payload = BlocksPayload {
            channel: "#general".to_string(),
            text: "test".to_string(),
            blocks: vec![Block::Section(SectionBlock::new("test"))],
        };
        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert!(json.get("attachments").is_none());
    }
}
