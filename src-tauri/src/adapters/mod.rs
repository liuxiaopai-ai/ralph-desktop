use crate::storage::models::CliType;
use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub mod claude;
pub mod codex;
pub mod opencode;

/// Parsed output line from CLI
#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub content: String,
    pub line_type: LineType,
    pub is_assistant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Text,
    Json,
    Error,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandOptions {
    pub skip_git_repo_check: bool,
}

/// CLI adapter trait for different CLI implementations
#[async_trait]
pub trait CliAdapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;

    /// CLI type
    fn cli_type(&self) -> CliType;

    /// Check if CLI is installed
    fn is_installed(&self) -> bool;

    /// Get CLI executable path
    fn get_path(&self) -> Option<String>;

    /// Get CLI version
    async fn version(&self) -> Option<String>;

    /// Build command for execution
    fn build_command(&self, prompt: &str, working_dir: &Path, options: CommandOptions) -> Command;

    /// Build readonly command for brainstorm
    fn build_readonly_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        options: CommandOptions,
    ) -> Command;

    /// Detect completion signal in output
    fn detect_completion(&self, output: &str, signal: &str) -> bool;

    /// Parse a single output line
    fn parse_output_line(&self, line: &str) -> ParsedLine;
}

/// Get all available CLI adapters
pub fn get_adapters() -> Vec<Box<dyn CliAdapter>> {
    vec![
        Box::new(claude::ClaudeCodeAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
    ]
}

/// Detect all installed CLIs
pub async fn detect_installed_clis() -> Vec<crate::storage::models::CliInfo> {
    let adapters = get_adapters();
    let mut results = Vec::new();

    for adapter in adapters {
        let available = adapter.is_installed();
        let path = adapter.get_path().unwrap_or_default();
        let version = if available {
            adapter.version().await
        } else {
            None
        };

        results.push(crate::storage::models::CliInfo {
            cli_type: adapter.cli_type(),
            name: adapter.name().to_string(),
            version,
            path,
            available,
        });
    }

    results
}

/// Get adapter for a specific CLI type
pub fn get_adapter(cli_type: CliType) -> Box<dyn CliAdapter> {
    match cli_type {
        CliType::Claude => Box::new(claude::ClaudeCodeAdapter::new()),
        CliType::Codex => Box::new(codex::CodexAdapter::new()),
        CliType::OpenCode => Box::new(opencode::OpenCodeAdapter::new()),
    }
}
