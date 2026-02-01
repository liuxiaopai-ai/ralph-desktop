use super::{
    apply_extended_path, apply_shell_env, command_for_cli, hide_console_window, resolve_cli_path,
    CliAdapter, CommandOptions, LineType, ParsedLine,
};
use crate::storage::models::CliType;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub struct ClaudeCodeAdapter {
    path: Option<String>,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let path = resolve_cli_path("claude");
        Self { path }
    }
}

#[async_trait]
impl CliAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &str {
        "Claude Code"
    }

    fn cli_type(&self) -> CliType {
        CliType::Claude
    }

    fn is_installed(&self) -> bool {
        self.path.is_some()
    }

    fn get_path(&self) -> Option<String> {
        self.path.clone()
    }

    async fn version(&self) -> Option<String> {
        let exe = self.path.as_deref().unwrap_or("claude");
        let mut cmd = Command::new(exe);
        apply_extended_path(&mut cmd);
        apply_shell_env(&mut cmd);
        hide_console_window(&mut cmd);
        let output = cmd.arg("--version").output().await.ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    fn build_command(&self, prompt: &str, working_dir: &Path, _options: CommandOptions) -> Command {
        let exe = self.path.as_deref().unwrap_or("claude");
        let mut args = vec![
            "--print".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
            "--verbose".to_string(),
        ];
        #[cfg(target_os = "windows")]
        {
            let _ = prompt;
            args.push("--input-format".to_string());
            args.push("text".to_string());
        }
        #[cfg(not(target_os = "windows"))]
        {
            args.push(prompt.to_string());
        }
        args.push("--output-format".to_string());
        args.push("stream-json".to_string());
        args.push("--include-partial-messages".to_string());
        let mut cmd = command_for_cli(exe, &args, working_dir);
        apply_extended_path(&mut cmd);
        apply_shell_env(&mut cmd);
        #[cfg(target_os = "windows")]
        {
            cmd.stdin(Stdio::piped());
        }
        #[cfg(not(target_os = "windows"))]
        {
            cmd.stdin(Stdio::null());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    fn build_readonly_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        _options: CommandOptions,
    ) -> Command {
        let exe = self.path.as_deref().unwrap_or("claude");
        let mut args = vec![
            "--print".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
            "--verbose".to_string(),
        ];
        #[cfg(target_os = "windows")]
        {
            let _ = prompt;
            args.push("--input-format".to_string());
            args.push("text".to_string());
        }
        #[cfg(not(target_os = "windows"))]
        {
            args.push(prompt.to_string());
        }
        args.push("--output-format".to_string());
        args.push("stream-json".to_string());
        args.push("--include-partial-messages".to_string());
        let mut cmd = command_for_cli(exe, &args, working_dir);
        apply_extended_path(&mut cmd);
        apply_shell_env(&mut cmd);
        #[cfg(target_os = "windows")]
        {
            cmd.stdin(Stdio::piped());
        }
        #[cfg(not(target_os = "windows"))]
        {
            cmd.stdin(Stdio::null());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    fn detect_completion(&self, output: &str, signal: &str) -> bool {
        for line in output.lines() {
            let parsed = self.parse_output_line(line);
            if parsed.is_assistant && parsed.content.contains(signal) {
                return true;
            }
        }
        false
    }

    fn parse_output_line(&self, line: &str) -> ParsedLine {
        // Try to parse as JSON first
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            let mut content = extract_text(&value).unwrap_or_default();
            let role = value.get("role").and_then(|v| v.as_str());
            let mut is_assistant = role == Some("assistant");
            if role.is_none() {
                let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if event_type.contains("message")
                    || event_type.contains("content")
                    || event_type.contains("assistant")
                {
                    is_assistant = true;
                }
            }
            if content.trim().is_empty() {
                // If content extraction failed but it's a valid JSON, use the raw line
                // unless it's a known non-content message type
                let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if event_type != "ping" && event_type != "progress" {
                    content = line.to_string();
                }
            }

            ParsedLine {
                content,
                line_type: LineType::Json,
                is_assistant,
            }
        } else {
            // Fallback for non-JSON lines
            // Some CLIs might output plain text debug info or partial JSON
            ParsedLine {
                content: line.to_string(),
                line_type: LineType::Text,
                is_assistant: false,
            }
        }
    }
}

fn extract_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    if let Some(content) = value.get("content") {
        if let Some(text) = content.as_str() {
            return Some(text.to_string());
        }
        if let Some(text) = join_text_array(content) {
            return Some(text);
        }
    }
    if let Some(delta) = value.get("delta") {
        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = join_text_array(delta.get("content").unwrap_or(delta)) {
            return Some(text);
        }
    }
    if let Some(message) = value.get("message") {
        if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(content) = message.get("content") {
            if let Some(text) = join_text_array(content) {
                return Some(text);
            }
        }
    }
    None
}

fn join_text_array(value: &Value) -> Option<String> {
    let array = value.as_array()?;
    let mut parts = Vec::new();
    for item in array {
        if let Some(text) = item.as_str() {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
            continue;
        }
        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        } else if let Some(text) = item.get("content").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::{ClaudeCodeAdapter, LineType};
    use crate::adapters::CliAdapter;

    #[test]
    fn parse_assistant_json_line() {
        let adapter = ClaudeCodeAdapter::new();
        let line = r#"{"type":"message","role":"assistant","content":"Hello"}"#;
        let parsed = adapter.parse_output_line(line);
        assert_eq!(parsed.content, "Hello");
        assert_eq!(parsed.line_type, LineType::Json);
        assert!(parsed.is_assistant);
    }

    #[test]
    fn parse_non_json_line() {
        let adapter = ClaudeCodeAdapter::new();
        let parsed = adapter.parse_output_line("plain text");
        assert_eq!(parsed.content, "plain text");
        assert_eq!(parsed.line_type, LineType::Text);
        assert!(!parsed.is_assistant);
    }
}
