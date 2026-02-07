use clap::Parser;
use slack_cli::slack::HttpSlackClient;
use slack_cli::token::resolve_token;
use slack_cli::{send_message, SendConfig, SlackCliError};
use std::io::{self, IsTerminal, Read};
use std::process;

#[derive(Parser)]
#[command(name = "slack-cli", about = "Send messages to Slack")]
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

fn run() -> Result<(), SlackCliError> {
    let args = Args::parse();

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

    let token = resolve_token()?;

    let config = SendConfig {
        channel: args.channel,
        message,
        color: args.color,
        token,
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
