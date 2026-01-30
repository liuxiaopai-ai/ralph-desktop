use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// AI brainstorm response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiBrainstormResponse {
    /// The AI's response text (question or final prompt)
    pub message: String,
    /// Whether brainstorming is complete
    pub is_complete: bool,
    /// The generated prompt (only when is_complete is true)
    pub generated_prompt: Option<String>,
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

const BRAINSTORM_SYSTEM_PROMPT: &str = r#"你是一个帮助用户明确编程任务需求的助手。你的目标是通过对话了解用户想要完成的任务，收集足够的信息后生成一个完整的任务 prompt。

## 你的工作方式

1. 首先理解用户的初始描述
2. 提出 1-2 个关键问题来澄清需求（不要一次问太多）
3. 根据用户回答，决定是否需要继续追问
4. 当你认为已经收集到足够信息时，生成最终的任务 prompt

## 问题类型参考

- 任务类型：新项目/添加功能/重构/修复bug？
- 技术栈：使用什么语言/框架？
- 具体功能：需要实现哪些具体功能？
- 测试要求：需要写测试吗？
- 其他约束：有什么特殊要求？

## 输出格式

如果还需要继续提问，直接输出你的问题。

如果已经收集够信息，请输出：

<brainstorm_complete>
[在这里输出完整的任务 prompt，包括：
- 任务描述
- 技术要求
- 具体功能列表
- 完成标准
- 完成信号：<done>COMPLETE</done>
]
</brainstorm_complete>

请用简洁友好的中文与用户对话。"#;

/// Run AI brainstorm with Claude Code
pub async fn run_ai_brainstorm(
    working_dir: &Path,
    conversation: &[ConversationMessage],
) -> Result<AiBrainstormResponse, String> {
    // Build the conversation context
    let mut context = String::new();

    for msg in conversation {
        if msg.role == "user" {
            context.push_str(&format!("用户: {}\n\n", msg.content));
        } else {
            context.push_str(&format!("助手: {}\n\n", msg.content));
        }
    }

    // Create the prompt for Claude
    let prompt = format!(
        "{}\n\n## 当前对话\n\n{}\n\n请继续对话，提出问题或生成最终 prompt。",
        BRAINSTORM_SYSTEM_PROMPT,
        context
    );

    // Call Claude Code CLI
    let output = call_claude_cli(working_dir, &prompt).await?;

    // Parse the response
    if output.contains("<brainstorm_complete>") {
        // Extract the generated prompt
        let start = output.find("<brainstorm_complete>")
            .map(|i| i + "<brainstorm_complete>".len())
            .unwrap_or(0);
        let end = output.find("</brainstorm_complete>").unwrap_or(output.len());
        let generated_prompt = output[start..end].trim().to_string();

        Ok(AiBrainstormResponse {
            message: "好的，我已经了解了你的需求。以下是生成的任务 prompt：".to_string(),
            is_complete: true,
            generated_prompt: Some(generated_prompt),
        })
    } else {
        Ok(AiBrainstormResponse {
            message: output.trim().to_string(),
            is_complete: false,
            generated_prompt: None,
        })
    }
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
