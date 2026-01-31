use crate::adapters::{get_adapter, CommandOptions, LineType};
use crate::storage::models::CliType;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
#[cfg(target_os = "windows")]
use tokio::io::AsyncWriteExt;

/// AI brainstorm response with structured options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiBrainstormResponse {
    /// The question text
    pub question: String,
    /// Optional description
    pub description: Option<String>,
    /// Available options (empty for text input)
    pub options: Vec<QuestionOption>,
    /// Whether multiple options can be selected
    pub multi_select: bool,
    /// Whether to show "Other" option for custom input
    pub allow_other: bool,
    /// Whether brainstorming is complete
    pub is_complete: bool,
    /// The generated prompt (only when is_complete is true)
    pub generated_prompt: Option<String>,
}

/// Question option
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
    pub value: String,
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

const BRAINSTORM_SYSTEM_PROMPT: &str = r#"You are a thought partner for programming tasks, helping users explore and clarify what they want to accomplish.

## Language Rule
IMPORTANT: Detect and match the user's language automatically. If the user writes in Chinese, respond in Chinese. If in English, respond in English. If in Japanese, respond in Japanese. Always mirror the user's language.

## Core Principles

1. **Collaborative Dialogue**: You are a thought partner, not a questionnaire. Explore together with the user, don't just mechanically collect information.
2. **Intellectual Curiosity**: Show genuine interest in the user's ideas, ask exploratory questions.
3. **Creative Challenge**: Push the user to think deeper, challenge assumptions, explore "what if..." scenarios.
4. **Structured yet Flexible**: Guide the conversation with purpose, but adapt dynamically based on the user's thinking.

## Workflow

### Phase 1: Understanding Context
Use open-ended questions to understand what the user is working on:
- "What problem are you trying to solve?"
- "What excites you most about this project?"
- "What's unsatisfying about existing solutions?"

### Phase 2: Divergent Exploration
Help the user think from multiple angles:
- Challenge assumptions: "What if you did it the opposite way?"
- Cross-domain analogies: "How do other fields solve similar problems?"
- Constraint thinking: "What if this limitation didn't exist?"

### Phase 3: Focus on Solution
When enough information is gathered, help the user focus:
- Confirm core features
- Confirm technical choices
- Confirm success criteria
- Confirm testing & validation plan (must ask at least one question)

### Phase 4: Generate Prompt
Synthesize all information into a complete task description.

## Output Format

Output strictly in JSON format, nothing else.

### Question with options (for clear choices):
```json
{
  "question": "Exploratory question",
  "description": "Optional description or your observation",
  "options": [
    {"label": "Option", "description": "Explanation", "value": "value"}
  ],
  "multiSelect": false,
  "allowOther": true,
  "isComplete": false
}
```

### Multi-select question (for features/characteristics):
```json
{
  "question": "Which features would you like?",
  "description": "You can select multiple",
  "options": [...],
  "multiSelect": true,
  "allowOther": true,
  "isComplete": false
}
```

### Open-ended question (no options):
```json
{
  "question": "Open-ended question",
  "description": "Guidance or context",
  "options": [],
  "multiSelect": false,
  "allowOther": false,
  "isComplete": false
}
```

### Completion:
```json
{
  "question": "Great, I understand your requirements",
  "description": "Let me summarize...",
  "options": [],
  "multiSelect": false,
  "allowOther": false,
  "isComplete": true,
  "generatedPrompt": "Complete task description..."
}
```

## Question Design Tips

### Good questions (exploratory, open-ended):
- "What problem are you trying to solve? What are the pain points with existing solutions?"
- "Who is this for? What do they care about most?"
- "If you could only implement one core feature, what would it be?"
- "Is there a product you really like that we can reference?"
- "When it's done, how will you know it's successful?"

### Questions to avoid (mechanical, closed):
- "What type of task is this?" ❌
- "What tech stack?" ❌ (unless user mentions technical choices)
- "Do you need tests?" ❌ (too early for details; ask later with context)

### When to use multi-select:
- Feature lists: "Which features would you like to include?"
- Pain point analysis: "What problems does the current solution have?"
- Target users: "Who are the main user groups?"
- Technical features: "What characteristics do you need to support?"

## Conversation Example

User: "I want to make a snake game"

Good response:
```json
{
  "question": "Interesting! What would make your snake game different?",
  "description": "Are you going for a classic recreation, or do you have unique ideas?",
  "options": [
    {"label": "Classic recreation", "description": "Faithfully reproduce traditional gameplay", "value": "classic"},
    {"label": "Add new mechanics", "description": "Innovate on the classic foundation", "value": "innovative"},
    {"label": "Complete redesign", "description": "Keep the core concept but innovate boldly", "value": "redesign"}
  ],
  "multiSelect": false,
  "allowOther": true,
  "isComplete": false
}
```

## Requirements for Generated Prompt

The final prompt should include:
1. **Task Overview**: One sentence description
2. **Background & Goals**: Why do this, what effect to achieve
3. **Core Features**: List of must-have features
4. **Technical Requirements**: Tech stack, constraints
5. **Testing & Validation**:
   - **Test Plan**: Must include at least unit tests; prefer E2E if applicable
   - **Test Commands**: Exact commands to run
   - **Manual Checks**: Only if automation is not feasible, with reasons
6. **Success Criteria**: Must include tests passing (or explicit exceptions)
7. **Completion Signal**: `<done>COMPLETE</done>`

## Mandatory Testing Rule
Before completing, you MUST ask about testing/validation. If the user is unsure, propose a default plan:
- At minimum: unit tests covering key logic
- If there is UI or end-to-end flow: add a minimal E2E smoke test

Remember: Match the user's language in all your responses!"#;

/// Run AI brainstorm with Claude Code
pub async fn run_ai_brainstorm(
    working_dir: &Path,
    conversation: &[ConversationMessage],
    cli_type: CliType,
    skip_git_repo_check: bool,
) -> Result<AiBrainstormResponse, String> {
    // Build the conversation context
    let mut context = String::new();

    for msg in conversation {
        if msg.role == "user" {
            context.push_str(&format!("User: {}\n\n", msg.content));
        } else {
            context.push_str(&format!("Assistant: {}\n\n", msg.content));
        }
    }

    // Create the prompt for Claude
    let prompt = format!(
        "{}\n\n## Conversation\n\n{}\n\nBased on the conversation above, output the next question JSON (or the final prompt). Output JSON only.",
        BRAINSTORM_SYSTEM_PROMPT,
        context
    );

    // Call Claude Code CLI
    let output = call_brainstorm_cli(cli_type, working_dir, &prompt, skip_git_repo_check).await?;

    // Parse JSON response
    parse_ai_response(&output)
}

/// Parse AI response JSON
fn parse_ai_response(output: &str) -> Result<AiBrainstormResponse, String> {
    // Try to extract JSON from the output
    match extract_json(output) {
        Ok(json_str) => {
            // Parse the JSON
            serde_json::from_str::<AiBrainstormResponse>(&json_str)
                .map_err(|e| format!("Failed to parse AI response: {}. Raw: {}", e, json_str))
        }
        Err(_) => {
            // If no JSON found, treat the output as a plain text question
            // This is a fallback for when AI doesn't follow JSON format
            let trimmed = output.trim();

            // Check if it looks like a completion
            if trimmed.contains("<done>COMPLETE</done>") {
                let (question, description) = match detect_language(trimmed) {
                    DetectedLanguage::Zh => (
                        "需求收集完成".to_string(),
                        "已生成任务 prompt".to_string(),
                    ),
                    DetectedLanguage::Ja => (
                        "要件確定".to_string(),
                        "タスクの prompt を生成しました".to_string(),
                    ),
                    DetectedLanguage::Ko => (
                        "요구사항 완료".to_string(),
                        "작업 prompt가 생성되었습니다".to_string(),
                    ),
                    DetectedLanguage::Other => (
                        "Requirements complete".to_string(),
                        "Generated task prompt".to_string(),
                    ),
                };
                Ok(AiBrainstormResponse {
                    question,
                    description: Some(description),
                    options: vec![],
                    multi_select: false,
                    allow_other: false,
                    is_complete: true,
                    generated_prompt: Some(trimmed.to_string()),
                })
            } else {
                // Treat as a plain text question
                Ok(AiBrainstormResponse {
                    question: trimmed.to_string(),
                    description: None,
                    options: vec![],
                    multi_select: false,
                    allow_other: false,
                    is_complete: false,
                    generated_prompt: None,
                })
            }
        }
    }
}

/// Extract JSON from output (handles markdown code blocks)
fn extract_json(output: &str) -> Result<String, String> {
    let trimmed = output.trim();

    // Try to find JSON in code block
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return Ok(trimmed[json_start..json_start + end].trim().to_string());
        }
    }

    // Try to find JSON in generic code block
    if let Some(start) = trimmed.find("```") {
        let block_start = start + 3;
        // Skip language identifier if present
        let json_start = if let Some(newline) = trimmed[block_start..].find('\n') {
            block_start + newline + 1
        } else {
            block_start
        };
        if let Some(end) = trimmed[json_start..].find("```") {
            return Ok(trimmed[json_start..json_start + end].trim().to_string());
        }
    }

    // Try to find raw JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return Ok(trimmed[start..=end].to_string());
        }
    }

    Err(format!("No JSON found in output: {}", output))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedLanguage {
    Zh,
    Ja,
    Ko,
    Other,
}

fn detect_language(input: &str) -> DetectedLanguage {
    if contains_hangul(input) {
        return DetectedLanguage::Ko;
    }
    if contains_kana(input) {
        return DetectedLanguage::Ja;
    }
    if contains_cjk(input) {
        return DetectedLanguage::Zh;
    }
    DetectedLanguage::Other
}

fn contains_kana(input: &str) -> bool {
    input.chars().any(|ch| {
        ('\u{3040}'..='\u{309F}').contains(&ch)
            || ('\u{30A0}'..='\u{30FF}').contains(&ch)
            || ('\u{31F0}'..='\u{31FF}').contains(&ch)
    })
}

fn contains_hangul(input: &str) -> bool {
    input.chars().any(|ch| ('\u{AC00}'..='\u{D7AF}').contains(&ch))
}

fn contains_cjk(input: &str) -> bool {
    input.chars().any(|ch| {
        ('\u{4E00}'..='\u{9FFF}').contains(&ch) || ('\u{3400}'..='\u{4DBF}').contains(&ch)
    })
}

/// Call Claude Code CLI and get response
async fn call_claude_cli(working_dir: &Path, prompt: &str) -> Result<String, String> {
    let exe = crate::adapters::resolve_cli_path("claude").unwrap_or_else(|| "claude".to_string());
    let mut args = vec![
        "--print".to_string(),
        "--dangerously-skip-permissions".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ];
    #[cfg(target_os = "windows")]
    {
        args.push("--input-format".to_string());
        args.push("text".to_string());
    }
    #[cfg(not(target_os = "windows"))]
    {
        args.push(prompt.to_string());
    }
    args.push("--output-format".to_string());
    args.push("text".to_string());
    let mut cmd = crate::adapters::command_for_cli(&exe, &args, working_dir);
    crate::adapters::apply_extended_path(&mut cmd);
    crate::adapters::apply_shell_env(&mut cmd);
    #[cfg(target_os = "windows")]
    {
        cmd.stdin(Stdio::piped());
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd.stdin(Stdio::null());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    let output = {
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to run claude: {}", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .map_err(|e| format!("Failed to write Claude prompt: {}", e))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| format!("Failed to write Claude prompt: {}", e))?;
        }
        child
            .wait_with_output()
            .await
            .map_err(|e| format!("Failed to run claude: {}", e))?
    };
    #[cfg(not(target_os = "windows"))]
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run claude: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let message = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("Claude CLI exited with status: {}", output.status)
        };
        return Err(message);
    }

    if stdout.trim().is_empty() && !stderr.trim().is_empty() {
        return Err(stderr.trim().to_string());
    }

    Ok(stdout)
}

async fn call_brainstorm_cli(
    cli_type: CliType,
    working_dir: &Path,
    prompt: &str,
    skip_git_repo_check: bool,
) -> Result<String, String> {
    match cli_type {
        CliType::Claude => call_claude_cli(working_dir, prompt).await,
        CliType::Codex | CliType::OpenCode => {
            call_other_cli(cli_type, working_dir, prompt, skip_git_repo_check).await
        }
    }
}

async fn call_other_cli(
    cli_type: CliType,
    working_dir: &Path,
    prompt: &str,
    skip_git_repo_check: bool,
) -> Result<String, String> {
    let adapter = get_adapter(cli_type);
    let options = CommandOptions { skip_git_repo_check };
    let mut cmd = adapter.build_readonly_command(prompt, working_dir, options);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run CLI: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let message = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("CLI exited with status: {}", output.status)
        };
        return Err(message);
    }

    let (text, stream_error) = collect_brainstorm_output(cli_type, &stdout);
    if !text.trim().is_empty() {
        return Ok(text);
    }
    if let Some(error) = stream_error {
        return Err(error);
    }
    if !stderr.trim().is_empty() {
        return Err(stderr.trim().to_string());
    }
    Ok(stdout)
}

fn collect_brainstorm_output(cli_type: CliType, stdout: &str) -> (String, Option<String>) {
    let adapter = get_adapter(cli_type);
    let mut text = String::new();
    let mut error = None;

    for line in stdout.lines() {
        let parsed = adapter.parse_output_line(line);

        if parsed.line_type == LineType::Error {
            if error.is_none() && !parsed.content.trim().is_empty() {
                error = Some(parsed.content.trim().to_string());
            }
            continue;
        }

        if parsed.content.trim().is_empty() {
            continue;
        }

        if parsed.line_type == LineType::Json && parsed.content == line {
            continue;
        }

        if parsed.line_type == LineType::Text {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(parsed.content.trim_end());
        } else {
            text.push_str(parsed.content.as_str());
        }
    }

    (text, error)
}

#[cfg(test)]
mod tests {
    use super::ConversationMessage;
    use crate::storage;
    use crate::storage::models::{BrainstormState, CliType, GlobalConfig, ProjectState, ProjectStatus};
    use chrono::Utc;
    use std::env;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let prev = env::var_os(key);
            env::set_var(key, value);
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.take() {
                env::set_var(self.key, prev);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn brainstorm_uses_configured_cli() {
        let temp_dir = TempDir::new().unwrap();
        let home_dir = temp_dir.path();
        let _home_guard = EnvVarGuard::set("HOME", home_dir);
        let _shell_guard = EnvVarGuard::set("SHELL", "/bin/bash");

        let bin_dir = home_dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        let codex_script = r#"#!/usr/bin/env bash
echo '{"question":"Hi","description":null,"options":[],"multiSelect":false,"allowOther":false,"isComplete":false}'
"#;
        write_executable(&bin_dir.join("codex"), codex_script);

        let claude_script = r#"#!/usr/bin/env bash
echo 'not-json'
"#;
        write_executable(&bin_dir.join("claude"), claude_script);

        let current_path = env::var_os("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), current_path.to_string_lossy());
        let _path_guard = EnvVarGuard::set("PATH", new_path);

        let project_path = home_dir.join("project");
        fs::create_dir_all(&project_path).unwrap();

        let mut config = GlobalConfig::default();
        config.default_cli = CliType::Codex;
        storage::save_config(&config).unwrap();

        let now = Utc::now();
        let project_id = Uuid::new_v4();
        let project_state = ProjectState {
            id: project_id,
            name: "Test".to_string(),
            path: project_path.to_string_lossy().to_string(),
            status: ProjectStatus::Brainstorming,
            skip_git_repo_check: false,
            brainstorm: Some(BrainstormState {
                answers: vec![],
                completed_at: None,
            }),
            task: None,
            execution: None,
            created_at: now,
            updated_at: now,
        };
        storage::save_project_state(&project_state).unwrap();

        let response = crate::commands::ai_brainstorm_chat(
            project_id.to_string(),
            vec![ConversationMessage {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
        )
        .await
        .unwrap();

        assert_eq!(response.question, "Hi");
    }
}
