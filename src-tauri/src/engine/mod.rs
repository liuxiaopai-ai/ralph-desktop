use crate::adapters::{get_adapter, CommandOptions};
use crate::storage::models::CliType;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "windows")]
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use crate::adapters::hide_console_window;
use tokio::sync::Notify;

pub mod ai_brainstorm;
pub mod logs;

pub const CODEX_GIT_REPO_CHECK_REQUIRED: &str = "codex_git_repo_check_required";

/// Loop events sent to frontend
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LoopEvent {
    #[serde(rename_all = "camelCase")]
    IterationStart { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    Output {
        project_id: String,
        iteration: u32,
        content: String,
        is_stderr: bool,
    },
    #[serde(rename_all = "camelCase")]
    Pausing { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    Paused { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    Resumed { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    Completed { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    MaxIterationsReached { project_id: String, iteration: u32 },
    #[serde(rename_all = "camelCase")]
    Error {
        project_id: String,
        iteration: u32,
        error: String,
    },
    #[serde(rename_all = "camelCase")]
    Stopped { project_id: String },
}

/// Loop engine state
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Running { iteration: u32 },
    Pausing { iteration: u32 },
    Paused { iteration: u32 },
    Completed { iteration: u32 },
    MaxIterationsReached { iteration: u32 },
    Failed { iteration: u32 },
}

/// Ralph Loop execution engine
pub struct LoopEngine {
    project_id: String,
    project_path: PathBuf,
    cli_type: CliType,
    prompt: String,
    max_iterations: u32,
    auto_commit: bool,
    completion_signal: String,
    iteration_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
    skip_git_repo_check: bool,
    pause_requested: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    resume_notify: Arc<Notify>,
    app_handle: AppHandle,
}

#[allow(dead_code)]
impl LoopEngine {
    pub fn new(
        project_id: String,
        project_path: PathBuf,
        cli_type: CliType,
        prompt: String,
        max_iterations: u32,
        auto_commit: bool,
        completion_signal: String,
        iteration_timeout: Option<Duration>,
        idle_timeout: Option<Duration>,
        skip_git_repo_check: bool,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            project_id,
            project_path,
            cli_type,
            prompt,
            max_iterations,
            auto_commit,
            completion_signal,
            iteration_timeout,
            idle_timeout,
            skip_git_repo_check,
            pause_requested: Arc::new(AtomicBool::new(false)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            app_handle,
        }
    }

    fn is_codex_git_repo_check_error(&self, line: &str) -> bool {
        self.cli_type == CliType::Codex
            && line.contains("Not inside a trusted directory")
            && line.contains("skip-git-repo-check")
    }

    fn emit_event(&self, event: LoopEvent) {
        let _ = self.app_handle.emit("loop-event", &event);
    }

    async fn commit_iteration_if_needed(&self, iteration: u32) -> Result<(), String> {
        if !self.auto_commit {
            return Ok(());
        }

        if !self.is_git_repo().await? {
            return Ok(());
        }

        let status = self.run_git(&["status", "--porcelain"]).await?;
        if status.trim().is_empty() {
            return Ok(());
        }

        let diff_stat = self.run_git(&["diff", "--stat"]).await.unwrap_or_default();
        let diff_full = self.run_git(&["diff"]).await.unwrap_or_default();
        let diff = Self::truncate_for_prompt(&diff_full, 4000);

        let message = match self.generate_commit_message(iteration, &diff_stat, &diff).await {
            Ok(msg) => msg,
            Err(_) => format!("ralph: iteration {}", iteration),
        };
        let message = Self::normalize_commit_message(&message, iteration);

        self.run_git(&["add", "-A"]).await?;
        let _ = self.run_git(&["commit", "-m", message.as_str()]).await?;
        Ok(())
    }

    async fn generate_commit_message(&self, iteration: u32, diff_stat: &str, diff: &str) -> Result<String, String> {
        let prompt = format!(
            "Generate a concise git commit message for iteration {iteration}.
Rules:
- Output only the commit message (single line).
- Max 72 characters.
- Use imperative mood.

Diff summary:
{diff_stat}

Diff:
{diff}
"
        );

        let adapter = get_adapter(self.cli_type);
        let options = CommandOptions {
            skip_git_repo_check: self.skip_git_repo_check,
        };
        let mut cmd = adapter.build_readonly_command(&prompt, &self.project_path, options);
        #[cfg(target_os = "windows")]
        let output = {
            if self.cli_type == CliType::Claude {
                let mut child = cmd.spawn().map_err(|e| format!("Failed to run CLI: {e}"))?;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin
                        .write_all(prompt.as_bytes())
                        .await
                        .map_err(|e| format!("Failed to write Claude prompt: {e}"))?;
                    stdin
                        .write_all(b"\n")
                        .await
                        .map_err(|e| format!("Failed to write Claude prompt: {e}"))?;
                }
                child
                    .wait_with_output()
                    .await
                    .map_err(|e| format!("Failed to run CLI: {e}"))?
            } else {
                cmd.output().await.map_err(|e| format!("Failed to run CLI: {e}"))?
            }
        };
        #[cfg(not(target_os = "windows"))]
        let output = cmd.output().await.map_err(|e| format!("Failed to run CLI: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Commit message generation failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    async fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.project_path).args(args);
        hide_console_window(&mut cmd);
        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to run git: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn is_git_repo(&self) -> Result<bool, String> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.project_path)
            .arg("rev-parse")
            .arg("--is-inside-work-tree");
        hide_console_window(&mut cmd);
        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to run git: {e}"))?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "true")
    }

    fn normalize_commit_message(raw: &str, iteration: u32) -> String {
        let mut line = raw
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .trim_matches('`')
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if line.is_empty() {
            line = format!("ralph: iteration {}", iteration);
        }
        if line.chars().count() > 72 {
            line = line.chars().take(72).collect();
        }
        line
    }

    fn truncate_for_prompt(input: &str, max_chars: usize) -> String {
        if input.chars().count() <= max_chars {
            return input.to_string();
        }
        let mut truncated: String = input.chars().take(max_chars).collect();
        truncated.push_str("\n... (truncated) ...");
        truncated
    }

    pub async fn start(&self) -> Result<LoopState, String> {
        let adapter = get_adapter(self.cli_type);
        let mut iteration = 0u32;

        // Reset flags
        self.stop_requested.store(false, Ordering::SeqCst);
        self.pause_requested.store(false, Ordering::SeqCst);

        while iteration < self.max_iterations {
            // Check stop request before iteration
            if self.stop_requested.load(Ordering::SeqCst) {
                self.emit_event(LoopEvent::Stopped {
                    project_id: self.project_id.clone(),
                });
                return Ok(LoopState::Idle);
            }

            // Check pause request before iteration
            if self.pause_requested.load(Ordering::SeqCst) {
                self.emit_event(LoopEvent::Paused {
                    project_id: self.project_id.clone(),
                    iteration,
                });

                // Wait for resume or stop
                loop {
                    tokio::select! {
                        _ = self.resume_notify.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            if self.stop_requested.load(Ordering::SeqCst) {
                                self.emit_event(LoopEvent::Stopped {
                                    project_id: self.project_id.clone(),
                                });
                                return Ok(LoopState::Idle);
                            }
                        }
                    }
                }

                self.pause_requested.store(false, Ordering::SeqCst);
                self.emit_event(LoopEvent::Resumed {
                    project_id: self.project_id.clone(),
                    iteration,
                });
            }

            iteration += 1;
            self.emit_event(LoopEvent::IterationStart {
                project_id: self.project_id.clone(),
                iteration,
            });

            let iteration_deadline = self.iteration_timeout.map(|timeout| Instant::now() + timeout);

            // Build and spawn command
            let options = CommandOptions {
                skip_git_repo_check: self.skip_git_repo_check,
            };
            let mut cmd = adapter.build_command(&self.prompt, &self.project_path, options);
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    self.emit_event(LoopEvent::Error {
                        project_id: self.project_id.clone(),
                        iteration,
                        error: format!("Failed to spawn CLI: {}", e),
                    });
                    continue;
                }
            };
            #[cfg(target_os = "windows")]
            if self.cli_type == CliType::Claude {
                if let Some(mut stdin) = child.stdin.take() {
                    if let Err(e) = stdin.write_all(self.prompt.as_bytes()).await {
                        let _ = child.kill().await;
                        self.emit_event(LoopEvent::Error {
                            project_id: self.project_id.clone(),
                            iteration,
                            error: format!("Failed to write Claude prompt: {}", e),
                        });
                        continue;
                    }
                    if let Err(e) = stdin.write_all(b"\n").await {
                        let _ = child.kill().await;
                        self.emit_event(LoopEvent::Error {
                            project_id: self.project_id.clone(),
                            iteration,
                            error: format!("Failed to write Claude prompt: {}", e),
                        });
                        continue;
                    }
                }
            }

            // Read stdout and stderr in parallel
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let mut stdout_reader = stdout.map(|s| BufReader::new(s).lines());
            let mut stderr_reader = stderr.map(|s| BufReader::new(s).lines());

            let mut stdout_done = stdout_reader.is_none();
            let mut stderr_done = stderr_reader.is_none();
            let mut last_output_time = Instant::now();
            let mut completed = false;

            while !stdout_done || !stderr_done {
                // Check stop request
                if self.stop_requested.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    self.emit_event(LoopEvent::Stopped {
                        project_id: self.project_id.clone(),
                    });
                    return Ok(LoopState::Idle);
                }

                tokio::select! {
                    // Read stdout
                    line = async {
                        if let Some(ref mut reader) = stdout_reader {
                            reader.next_line().await
                        } else {
                            Ok(None)
                        }
                    }, if !stdout_done => {
                        match line {
                            Ok(Some(line)) => {
                                last_output_time = Instant::now();
                                let parsed = adapter.parse_output_line(&line);

                                self.emit_event(LoopEvent::Output {
                                    project_id: self.project_id.clone(),
                                    iteration,
                                    content: parsed.content.clone(),
                                    is_stderr: false,
                                });

                                // Check completion signal
                                if parsed.is_assistant && parsed.content.contains(&self.completion_signal) {
                                    completed = true;
                                    let _ = child.kill().await;
                                    break;
                                }
                            }
                            Ok(None) => stdout_done = true,
                            Err(_) => stdout_done = true,
                        }
                    }

                    // Read stderr
                    line = async {
                        if let Some(ref mut reader) = stderr_reader {
                            reader.next_line().await
                        } else {
                            Ok(None)
                        }
                    }, if !stderr_done => {
                        match line {
                            Ok(Some(line)) => {
                                if self.is_codex_git_repo_check_error(&line) {
                                    self.emit_event(LoopEvent::Error {
                                        project_id: self.project_id.clone(),
                                        iteration,
                                        error: CODEX_GIT_REPO_CHECK_REQUIRED.to_string(),
                                    });
                                    let _ = child.kill().await;
                                    return Ok(LoopState::Failed { iteration });
                                }
                                last_output_time = Instant::now();
                                self.emit_event(LoopEvent::Output {
                                    project_id: self.project_id.clone(),
                                    iteration,
                                    content: line,
                                    is_stderr: self.cli_type != CliType::Codex,
                                });
                            }
                            Ok(None) => stderr_done = true,
                            Err(_) => stderr_done = true,
                        }
                    }

                    // Timeout check
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        let now = Instant::now();

                        // Iteration timeout
                        if let Some(deadline) = iteration_deadline {
                            if now >= deadline {
                                self.emit_event(LoopEvent::Error {
                                    project_id: self.project_id.clone(),
                                    iteration,
                                    error: format!("Iteration timeout: exceeded {:?}", self.iteration_timeout),
                                });
                                let _ = child.kill().await;
                                break;
                            }
                        }

                        // Idle timeout
                        if let Some(idle_timeout) = self.idle_timeout {
                            if now.duration_since(last_output_time) > idle_timeout {
                                self.emit_event(LoopEvent::Error {
                                    project_id: self.project_id.clone(),
                                    iteration,
                                    error: format!("Idle timeout: no output for {:?}", self.idle_timeout),
                                });
                                let _ = child.kill().await;
                                break;
                            }
                        }
                    }
                }
            }

            // Wait for process to finish
            let _ = child.wait().await;

            if let Err(err) = self.commit_iteration_if_needed(iteration).await {
                self.emit_event(LoopEvent::Output {
                    project_id: self.project_id.clone(),
                    iteration,
                    content: format!("[auto-commit] {}", err),
                    is_stderr: true,
                });
            }

            if completed {
                self.emit_event(LoopEvent::Completed {
                    project_id: self.project_id.clone(),
                    iteration,
                });
                return Ok(LoopState::Completed { iteration });
            }

            // Check pause after iteration
            if self.pause_requested.load(Ordering::SeqCst) {
                self.emit_event(LoopEvent::Paused {
                    project_id: self.project_id.clone(),
                    iteration,
                });

                loop {
                    tokio::select! {
                        _ = self.resume_notify.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            if self.stop_requested.load(Ordering::SeqCst) {
                                self.emit_event(LoopEvent::Stopped {
                                    project_id: self.project_id.clone(),
                                });
                                return Ok(LoopState::Idle);
                            }
                        }
                    }
                }

                self.pause_requested.store(false, Ordering::SeqCst);
                self.emit_event(LoopEvent::Resumed {
                    project_id: self.project_id.clone(),
                    iteration,
                });
            }
        }

        // Max iterations reached
        self.emit_event(LoopEvent::MaxIterationsReached {
            project_id: self.project_id.clone(),
            iteration,
        });

        Ok(LoopState::MaxIterationsReached { iteration })
    }

    pub fn pause(&self) {
        self.pause_requested.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.resume_notify.notify_one();
    }

    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    pub fn get_pause_flag(&self) -> Arc<AtomicBool> {
        self.pause_requested.clone()
    }

    pub fn get_stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_requested.clone()
    }

    pub fn get_resume_notify(&self) -> Arc<Notify> {
        self.resume_notify.clone()
    }
}
