use super::{
    apply_extended_path, apply_shell_env, command_for_cli, hide_console_window, resolve_cli_path,
    shell_env_has, shell_env_value, CliAdapter, CommandOptions, LineType, ParsedLine,
};
use crate::storage::models::CliType;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, ffi::OsStr};
use tokio::process::Command;

pub struct OpenCodeAdapter {
    path: Option<String>,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        let path = resolve_cli_path("opencode");
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
        let exe = self.path.as_deref().unwrap_or("opencode");
        let args = if readonly {
            Self::readonly_args(prompt)
        } else {
            Self::exec_args(prompt)
        };
        let mut cmd = command_for_cli(exe, &args, working_dir);
        apply_extended_path(&mut cmd);
        apply_shell_env(&mut cmd);
        Self::apply_full_access(&mut cmd);
        cmd.stdin(Stdio::null())
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

    fn apply_full_access(cmd: &mut Command) {
        if let Some(config) = load_opencode_config_content() {
            let merged = merge_permissions(config);
            cmd.env("OPENCODE_CONFIG_CONTENT", merged.to_string());
            return;
        }

        if has_env_key("OPENCODE_CONFIG_CONTENT") || shell_env_has("OPENCODE_CONFIG_CONTENT") {
            return;
        }

        if let Some(config) = load_opencode_config_file() {
            let merged = merge_permissions(config);
            cmd.env("OPENCODE_CONFIG_CONTENT", merged.to_string());
            return;
        }

        cmd.env(
            "OPENCODE_CONFIG_CONTENT",
            full_access_template().to_string(),
        );
    }
}

fn has_env_key(key: &str) -> bool {
    env::var_os(key).is_some()
        || env::vars_os().any(|(k, _)| k == OsStr::new(key))
}

fn env_or_shell(key: &str) -> Option<String> {
    env::var(key).ok().or_else(|| shell_env_value(key))
}

fn load_opencode_config_content() -> Option<Value> {
    let content = env_or_shell("OPENCODE_CONFIG_CONTENT")?;
    serde_json::from_str(&content).ok()
}

fn config_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(path) = env_or_shell("OPENCODE_CONFIG") {
        paths.push(PathBuf::from(path));
    }

    if let Some(dir) = env_or_shell("OPENCODE_CONFIG_DIR") {
        paths.push(PathBuf::from(dir).join("config.json"));
    }

    if let Some(xdg) = env_or_shell("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("opencode").join("config.json"));
    }

    if let Some(home) = env_or_shell("HOME").or_else(|| env_or_shell("USERPROFILE")) {
        let home_path = PathBuf::from(home);
        paths.push(home_path.join(".config/opencode/config.json"));
        paths.push(home_path.join(".opencode/config.json"));
    }

    #[cfg(target_os = "windows")]
    if let Some(appdata) = env_or_shell("APPDATA") {
        paths.push(PathBuf::from(appdata).join("opencode").join("config.json"));
    }

    paths
}

fn load_opencode_config_file() -> Option<Value> {
    for path in config_candidate_paths() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&contents) {
                return Some(value);
            }
        }
    }
    None
}

fn merge_permissions(config: Value) -> Value {
    let mut config = match config {
        Value::Object(_) => config,
        _ => json!({}),
    };

    let permission = full_access_permissions();
    apply_permissions(&mut config, "agent", &["general", "build", "plan", "explore"], &permission);
    apply_permissions(&mut config, "mode", &["build", "plan"], &permission);
    config
}

fn apply_permissions(config: &mut Value, section: &str, keys: &[&str], permission: &Value) {
    if !config.get(section).map(|v| v.is_object()).unwrap_or(false) {
        config[section] = json!({});
    }

    let Some(section_map) = config.get_mut(section).and_then(|v| v.as_object_mut()) else {
        return;
    };

    for key in keys {
        let entry = section_map
            .entry((*key).to_string())
            .or_insert_with(|| json!({}));
        if let Some(map) = entry.as_object_mut() {
            map.insert("permission".to_string(), permission.clone());
        } else {
            *entry = json!({ "permission": permission.clone() });
        }
    }
}

fn full_access_permissions() -> Value {
    json!({
        "edit": "allow",
        "bash": "allow",
        "webfetch": "allow",
        "doom_loop": "allow",
        "external_directory": "allow"
    })
}

fn full_access_template() -> Value {
    json!({
        "agent": {
            "general": { "permission": full_access_permissions() },
            "build": { "permission": full_access_permissions() },
            "plan": { "permission": full_access_permissions() },
            "explore": { "permission": full_access_permissions() }
        },
        "mode": {
            "build": { "permission": full_access_permissions() },
            "plan": { "permission": full_access_permissions() }
        }
    })
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
        let exe = self.path.as_deref().unwrap_or("opencode");
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
    use super::{LineType, OpenCodeAdapter};
    use crate::adapters::CliAdapter;

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

    #[test]
    fn parse_text_event() {
        let adapter = OpenCodeAdapter::new();
        let line = r#"{"type":"text","part":{"type":"text","text":"Hi"}}"#;
        let parsed = adapter.parse_output_line(line);
        assert_eq!(parsed.content, "Hi");
        assert_eq!(parsed.line_type, LineType::Json);
        assert!(parsed.is_assistant);
    }

    #[test]
    fn parse_error_event() {
        let adapter = OpenCodeAdapter::new();
        let line = r#"{"type":"error","error":{"message":"boom"}}"#;
        let parsed = adapter.parse_output_line(line);
        assert_eq!(parsed.content, "boom");
        assert_eq!(parsed.line_type, LineType::Error);
        assert!(!parsed.is_assistant);
    }
}
