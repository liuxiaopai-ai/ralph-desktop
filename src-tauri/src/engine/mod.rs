use crate::adapters::{get_adapter, CliAdapter};
use crate::storage::models::CliType;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;

pub mod ai_brainstorm;
pub mod brainstorm;
pub mod logs;

/// Loop events sent to frontend
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Running { iteration: u32 },
    Pausing { iteration: u32 },
    Paused { iteration: u32 },
    Completed { iteration: u32 },
    Failed { iteration: u32 },
}

/// Ralph Loop execution engine
pub struct LoopEngine {
    project_id: String,
    project_path: PathBuf,
    cli_type: CliType,
    prompt: String,
    max_iterations: u32,
    completion_signal: String,
    iteration_timeout: Duration,
    idle_timeout: Duration,
    pause_requested: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    resume_notify: Arc<Notify>,
    app_handle: AppHandle,
}

impl LoopEngine {
    pub fn new(
        project_id: String,
        project_path: PathBuf,
        cli_type: CliType,
        prompt: String,
        max_iterations: u32,
        completion_signal: String,
        iteration_timeout_ms: u64,
        idle_timeout_ms: u64,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            project_id,
            project_path,
            cli_type,
            prompt,
            max_iterations,
            completion_signal,
            iteration_timeout: Duration::from_millis(iteration_timeout_ms),
            idle_timeout: Duration::from_millis(idle_timeout_ms),
            pause_requested: Arc::new(AtomicBool::new(false)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            app_handle,
        }
    }

    fn emit_event(&self, event: LoopEvent) {
        let _ = self.app_handle.emit("loop-event", &event);
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

            let iteration_deadline = Instant::now() + self.iteration_timeout;

            // Build and spawn command
            let mut cmd = adapter.build_command(&self.prompt, &self.project_path);
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
                                last_output_time = Instant::now();
                                self.emit_event(LoopEvent::Output {
                                    project_id: self.project_id.clone(),
                                    iteration,
                                    content: line,
                                    is_stderr: true,
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
                        if now >= iteration_deadline {
                            self.emit_event(LoopEvent::Error {
                                project_id: self.project_id.clone(),
                                iteration,
                                error: format!("Iteration timeout: exceeded {:?}", self.iteration_timeout),
                            });
                            let _ = child.kill().await;
                            break;
                        }

                        // Idle timeout
                        if now.duration_since(last_output_time) > self.idle_timeout {
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

            if completed {
                self.emit_event(LoopEvent::Completed {
                    project_id: self.project_id.clone(),
                    iteration,
                });
                return Ok(LoopState::Completed { iteration });
            }

            // Wait for process to finish
            let _ = child.wait().await;

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

        Ok(LoopState::Failed { iteration })
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
