use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

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
- "Do you need tests?" ❌ (too early for details)

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
5. **Success Criteria**: How to judge completion
6. **Completion Signal**: `<done>COMPLETE</done>`

Remember: Match the user's language in all your responses!"#;

/// Run AI brainstorm with Claude Code
pub async fn run_ai_brainstorm(
    working_dir: &Path,
    conversation: &[ConversationMessage],
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
    let output = call_claude_cli(working_dir, &prompt).await?;

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
                let (question, description) = if contains_cjk(trimmed) {
                    ("需求收集完成".to_string(), "已生成任务 prompt".to_string())
                } else {
                    ("Requirements complete".to_string(), "Generated task prompt".to_string())
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

fn contains_cjk(input: &str) -> bool {
    input.chars().any(|ch| {
        ('\u{4E00}'..='\u{9FFF}').contains(&ch) || ('\u{3400}'..='\u{4DBF}').contains(&ch)
    })
}

/// Call Claude Code CLI and get response
async fn call_claude_cli(working_dir: &Path, prompt: &str) -> Result<String, String> {
    let mut cmd = Command::new("claude");
    cmd.arg("--print")
        .arg("--dangerously-skip-permissions")
        .arg("-p")
        .arg(prompt)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn claude: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let mut output = String::new();

    while let Some(line) = reader.next_line().await.map_err(|e| e.to_string())? {
        output.push_str(&line);
        output.push('\n');
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!("Claude CLI exited with status: {}", status));
    }

    Ok(output)
}
