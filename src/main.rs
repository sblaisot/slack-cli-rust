use clap::Parser;
use serde_json::Value;
use slack_cli::slack::HttpSlackClient;
use slack_cli::token::resolve_token;
use slack_cli::{send_message, SendConfig, SlackCliError};
use std::io::{self, IsTerminal, Read};
use std::process;

#[derive(Parser)]
#[command(
    name = "slack-cli",
    about = "Send messages to Slack",
    version,
    before_help = concat!("slack-cli v", env!("CARGO_PKG_VERSION")),
)]
struct Args {
    /// Channel name or ID (e.g. "#general" or "C01234567")
    #[arg(short, long)]
    channel: String,

    /// Message text (reads from stdin if omitted)
    #[arg(short, long)]
    message: Option<String>,

    /// Hex color for attachment sidebar (e.g. "#FF0000")
    #[arg(long)]
    color: Option<String>,

    /// Title displayed as a header above the message
    #[arg(short, long)]
    title: Option<String>,

    /// JSON blocks file (reads from stdin if omitted)
    #[arg(long, num_args = 0..=1, default_missing_value = "-", conflicts_with = "title")]
    blocks: Option<String>,
}

fn read_stdin() -> Result<String, SlackCliError> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(SlackCliError::StdinError)?;
    let trimmed = buffer.trim().to_string();
    if trimmed.is_empty() {
        return Err(SlackCliError::NoMessage);
    }
    Ok(trimmed)
}

fn parse_blocks_json(json_str: &str) -> Result<Vec<Value>, SlackCliError> {
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| SlackCliError::InvalidBlocksJson(e.to_string()))?;

    let arr = value
        .as_array()
        .ok_or_else(|| SlackCliError::InvalidBlocksJson("expected a JSON array".to_string()))?;

    if arr.is_empty() {
        return Err(SlackCliError::InvalidBlocksJson(
            "blocks array is empty".to_string(),
        ));
    }

    if arr.len() > 100 {
        return Err(SlackCliError::InvalidBlocksJson(
            "too many blocks (max 100)".to_string(),
        ));
    }

    for item in arr {
        if !item.is_object() {
            return Err(SlackCliError::InvalidBlocksJson(
                "each block must be a JSON object".to_string(),
            ));
        }
    }

    Ok(arr.clone())
}

fn read_blocks(source: &str) -> Result<Vec<Value>, SlackCliError> {
    let json_str = if source == "-" {
        if io::stdin().is_terminal() {
            return Err(SlackCliError::InvalidBlocksJson(
                "no input piped to stdin".to_string(),
            ));
        }
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(SlackCliError::StdinError)?;
        buffer
    } else {
        std::fs::read_to_string(source).map_err(|e| {
            SlackCliError::InvalidBlocksJson(format!("failed to read file '{source}': {e}"))
        })?
    };

    parse_blocks_json(&json_str)
}

fn run() -> Result<(), SlackCliError> {
    let args = Args::parse();

    let (message, blocks) = if let Some(source) = args.blocks {
        let blocks = read_blocks(&source)?;
        let message = args.message.unwrap_or_default();
        (message, Some(blocks))
    } else {
        let message = match args.message {
            Some(msg) if !msg.trim().is_empty() => msg,
            Some(_) => return Err(SlackCliError::NoMessage),
            None => {
                if io::stdin().is_terminal() {
                    return Err(SlackCliError::NoMessage);
                }
                read_stdin()?
            }
        };
        (message, None)
    };

    let token = resolve_token()?;

    let config = SendConfig {
        channel: args.channel,
        message,
        color: args.color,
        title: args.title,
        token,
        blocks,
    };

    let client = HttpSlackClient;
    let result = send_message(&client, &config)?;

    if let Some(warning) = result.warning {
        eprintln!("Warning: {warning}");
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_blocks_json_valid_array() {
        let json = r#"[{"type": "section", "text": {"type": "mrkdwn", "text": "Hello"}}]"#;
        let result = parse_blocks_json(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "section");
    }

    #[test]
    fn test_parse_blocks_json_multiple_blocks() {
        let json = r#"[{"type": "section", "text": {"type": "mrkdwn", "text": "Hello"}}, {"type": "divider"}]"#;
        let result = parse_blocks_json(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["type"], "section");
        assert_eq!(result[1]["type"], "divider");
    }

    #[test]
    fn test_parse_blocks_json_empty_array_rejected() {
        let result = parse_blocks_json("[]");
        assert!(matches!(
            result,
            Err(SlackCliError::InvalidBlocksJson(ref msg)) if msg.contains("empty")
        ));
    }

    #[test]
    fn test_parse_blocks_json_non_array_rejected() {
        let result = parse_blocks_json(r#"{"type": "section"}"#);
        assert!(matches!(
            result,
            Err(SlackCliError::InvalidBlocksJson(ref msg)) if msg.contains("array")
        ));
    }

    #[test]
    fn test_parse_blocks_json_non_object_elements_rejected() {
        let result = parse_blocks_json(r#"["not an object"]"#);
        assert!(matches!(
            result,
            Err(SlackCliError::InvalidBlocksJson(ref msg)) if msg.contains("object")
        ));
    }

    #[test]
    fn test_parse_blocks_json_invalid_json_rejected() {
        let result = parse_blocks_json("not json at all");
        assert!(matches!(result, Err(SlackCliError::InvalidBlocksJson(_))));
    }

    #[test]
    fn test_parse_blocks_json_too_many_blocks_rejected() {
        let blocks: Vec<Value> = (0..101)
            .map(|_| serde_json::json!({"type": "divider"}))
            .collect();
        let json = serde_json::to_string(&blocks).unwrap();
        let result = parse_blocks_json(&json);
        assert!(matches!(
            result,
            Err(SlackCliError::InvalidBlocksJson(ref msg)) if msg.contains("max 100")
        ));
    }

    #[test]
    fn test_parse_blocks_json_100_blocks_accepted() {
        let blocks: Vec<Value> = (0..100)
            .map(|_| serde_json::json!({"type": "divider"}))
            .collect();
        let json = serde_json::to_string(&blocks).unwrap();
        let result = parse_blocks_json(&json).unwrap();
        assert_eq!(result.len(), 100);
    }
}
