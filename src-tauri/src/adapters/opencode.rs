use super::{CliAdapter, CommandOptions, LineType, ParsedLine};
use crate::storage::models::CliType;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub struct OpenCodeAdapter {
    path: Option<String>,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        let path = which::which("opencode")
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        Self { path }
    }

    fn exec_args(prompt: &str) -> Vec<String> {
        vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            prompt.to_string(),
        ]
    }

    fn readonly_args(prompt: &str) -> Vec<String> {
        vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--agent".to_string(),
            "plan".to_string(),
            prompt.to_string(),
        ]
    }

    fn build_run_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        readonly: bool,
        _options: CommandOptions,
    ) -> Command {
        let mut cmd = Command::new("opencode");
        let args = if readonly {
            Self::readonly_args(prompt)
        } else {
            Self::exec_args(prompt)
        };
        cmd.current_dir(working_dir)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    fn extract_text(value: &Value) -> Option<String> {
        if let Some(text) = value.pointer("/part/text").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = value.pointer("/error/message").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = value.get("message").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = value.pointer("/data/message").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        None
    }
}

#[async_trait]
impl CliAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "OpenCode"
    }

    fn cli_type(&self) -> CliType {
        CliType::OpenCode
    }

    fn is_installed(&self) -> bool {
        self.path.is_some()
    }

    fn get_path(&self) -> Option<String> {
        self.path.clone()
    }

    async fn version(&self) -> Option<String> {
        let output = Command::new("opencode")
            .arg("--version")
            .output()
            .await
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    fn build_command(&self, prompt: &str, working_dir: &Path, options: CommandOptions) -> Command {
        self.build_run_command(prompt, working_dir, false, options)
    }

    fn build_readonly_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        options: CommandOptions,
    ) -> Command {
        self.build_run_command(prompt, working_dir, true, options)
    }

    fn detect_completion(&self, output: &str, signal: &str) -> bool {
        output.contains(signal)
    }

    fn parse_output_line(&self, line: &str) -> ParsedLine {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if event_type == "text" {
                return ParsedLine {
                    content: Self::extract_text(&value).unwrap_or_default(),
                    line_type: LineType::Json,
                    is_assistant: true,
                };
            }

            if event_type == "error" {
                return ParsedLine {
                    content: Self::extract_text(&value).unwrap_or_else(|| line.to_string()),
                    line_type: LineType::Error,
                    is_assistant: false,
                };
            }

            return ParsedLine {
                content: Self::extract_text(&value).unwrap_or_else(|| line.to_string()),
                line_type: LineType::Json,
                is_assistant: false,
            };
        }

        ParsedLine {
            content: line.to_string(),
            line_type: LineType::Text,
            is_assistant: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OpenCodeAdapter;

    #[test]
    fn exec_args_include_format_json() {
        let args = OpenCodeAdapter::exec_args("hello");
        assert_eq!(args, vec!["run", "--format", "json", "hello"]);
    }

    #[test]
    fn readonly_args_use_plan_agent() {
        let args = OpenCodeAdapter::readonly_args("hello");
        assert_eq!(
            args,
            vec!["run", "--format", "json", "--agent", "plan", "hello"]
        );
    }
}
