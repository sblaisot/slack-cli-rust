# slack-cli

A Rust CLI tool for sending messages to Slack.

## Installation

### From GitHub releases

Download the archive for your platform from the [releases page](https://github.com/sebastien/slack-cli-rust/releases), extract it, and move the binary to a directory in your `PATH`.

On macOS, the binary is not signed with an Apple Developer certificate. Gatekeeper will block it on first run. Remove the quarantine attribute to allow execution:

```sh
xattr -d com.apple.quarantine ./slack-cli
```

### Docker

```sh
docker run --rm -e SLACK_API_KEY ghcr.io/sblaisot/slack-cli-rust:latest \
  --channel "#general" --message "Hello from Docker"
```

Multi-arch images (`linux/amd64` and `linux/arm64`) are published to `ghcr.io` for every release tag.

### From source

```sh
cargo build --release
cp target/release/slack-cli /usr/local/bin/
```

## Configuration

Set your Slack API token using one of these methods (checked in order):

1. `SLACK_API_KEY` environment variable
2. `~/.slack/api-token` file
3. `/etc/slack/api-token` file

The token needs the `chat:write` scope.

## Usage

```sh
# Send a plain text message
slack-cli --channel "#general" --message "Hello world"

# Send with a title header
slack-cli --channel "#general" --message "Build passed" --title "CI Status"

# Send with a colored sidebar
slack-cli --channel "#general" --message "Build passed" --color "#36a64f"

# Pipe input from another command
echo "Deploy complete" | slack-cli --channel "#ops"

# Pipe a file
cat report.txt | slack-cli --channel "#reports"
```

### Options

| Flag | Short | Required | Description |
|------|-------|----------|-------------|
| `--channel` | `-c` | Yes | Channel name or ID |
| `--message` | `-m` | No | Message text (reads stdin if omitted) |
| `--color` | | No | Hex color for attachment sidebar |
| `--title` | `-t` | No | Title displayed as a header above the message |

## Message Format Behavior

| Condition | Format | Note |
|-----------|--------|------|
| No `--color` | Block Kit (plain text) | Modern Slack API |
| `--color`, message <= 4000 chars | Attachment with color sidebar | Only way to get color |
| `--color`, message > 4000 chars | Block Kit (no color) | Warning printed to stderr |

The 4000 character limit is a Slack API constraint on attachment text. When exceeded, the message is sent without color and a warning is printed to stderr.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Message sent successfully |
| 1 | Error (token not found, API error, no message, etc.) |
