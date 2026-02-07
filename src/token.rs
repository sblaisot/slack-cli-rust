use crate::SlackCliError;
use std::env;
use std::fs;
use std::path::PathBuf;

pub struct TokenConfig {
    pub env_var: String,
    pub file_paths: Vec<PathBuf>,
}

impl Default for TokenConfig {
    fn default() -> Self {
        let home = env::var("HOME").unwrap_or_default();
        TokenConfig {
            env_var: "SLACK_API_KEY".to_string(),
            file_paths: vec![
                PathBuf::from(format!("{home}/.slack/api-token")),
                PathBuf::from("/etc/slack/api-token"),
            ],
        }
    }
}

pub fn resolve_token() -> Result<String, SlackCliError> {
    resolve_token_with_config(&TokenConfig::default())
}

pub fn resolve_token_with_config(config: &TokenConfig) -> Result<String, SlackCliError> {
    if let Ok(token) = env::var(&config.env_var) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    for path in &config.file_paths {
        if let Ok(contents) = fs::read_to_string(path) {
            let token = contents.trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    Err(SlackCliError::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_env_var_resolution() {
        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_1".to_string(),
            file_paths: vec![],
        };
        env::set_var("SLACK_CLI_TEST_TOKEN_1", "xoxb-test-token");
        let result = resolve_token_with_config(&config);
        env::remove_var("SLACK_CLI_TEST_TOKEN_1");
        assert_eq!(result.unwrap(), "xoxb-test-token");
    }

    #[test]
    fn test_env_var_trimmed() {
        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_2".to_string(),
            file_paths: vec![],
        };
        env::set_var("SLACK_CLI_TEST_TOKEN_2", "  xoxb-test-token  ");
        let result = resolve_token_with_config(&config);
        env::remove_var("SLACK_CLI_TEST_TOKEN_2");
        assert_eq!(result.unwrap(), "xoxb-test-token");
    }

    #[test]
    fn test_file_resolution() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "xoxb-file-token").unwrap();

        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_NONEXISTENT".to_string(),
            file_paths: vec![tmp.path().to_path_buf()],
        };
        let result = resolve_token_with_config(&config);
        assert_eq!(result.unwrap(), "xoxb-file-token");
    }

    #[test]
    fn test_file_trimmed() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "  xoxb-file-token\n").unwrap();

        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_NONEXISTENT".to_string(),
            file_paths: vec![tmp.path().to_path_buf()],
        };
        let result = resolve_token_with_config(&config);
        assert_eq!(result.unwrap(), "xoxb-file-token");
    }

    #[test]
    fn test_missing_token() {
        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_NONEXISTENT".to_string(),
            file_paths: vec![PathBuf::from("/nonexistent/path/token")],
        };
        let result = resolve_token_with_config(&config);
        assert!(matches!(result, Err(SlackCliError::TokenNotFound)));
    }

    #[test]
    fn test_env_var_takes_priority_over_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "xoxb-file-token").unwrap();

        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_3".to_string(),
            file_paths: vec![tmp.path().to_path_buf()],
        };
        env::set_var("SLACK_CLI_TEST_TOKEN_3", "xoxb-env-token");
        let result = resolve_token_with_config(&config);
        env::remove_var("SLACK_CLI_TEST_TOKEN_3");
        assert_eq!(result.unwrap(), "xoxb-env-token");
    }

    #[test]
    fn test_first_file_takes_priority() {
        let mut tmp1 = NamedTempFile::new().unwrap();
        write!(tmp1, "xoxb-first").unwrap();
        let mut tmp2 = NamedTempFile::new().unwrap();
        write!(tmp2, "xoxb-second").unwrap();

        let config = TokenConfig {
            env_var: "SLACK_CLI_TEST_TOKEN_NONEXISTENT".to_string(),
            file_paths: vec![tmp1.path().to_path_buf(), tmp2.path().to_path_buf()],
        };
        let result = resolve_token_with_config(&config);
        assert_eq!(result.unwrap(), "xoxb-first");
    }
}
