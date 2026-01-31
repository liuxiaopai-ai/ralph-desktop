use crate::storage::{get_project_dir, ensure_project_dir};
use chrono::Utc;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Log manager for persisting execution logs
#[allow(dead_code)]
pub struct LogManager {
    project_id: uuid::Uuid,
    log_file: Option<BufWriter<File>>,
    log_path: Option<PathBuf>,
}

#[allow(dead_code)]
impl LogManager {
    pub fn new(project_id: uuid::Uuid) -> Self {
        Self {
            project_id,
            log_file: None,
            log_path: None,
        }
    }

    /// Start a new log session
    pub fn start_session(&mut self) -> Result<PathBuf, String> {
        let project_dir = ensure_project_dir(&self.project_id).map_err(|e| e.to_string())?;
        let logs_dir = project_dir.join("logs");
        fs::create_dir_all(&logs_dir).map_err(|e| e.to_string())?;

        let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        let log_path = logs_dir.join(format!("{}.log", timestamp));

        let file = File::create(&log_path).map_err(|e| e.to_string())?;
        self.log_file = Some(BufWriter::new(file));
        self.log_path = Some(log_path.clone());

        // Write header
        self.write_line(&format!("# Ralph Desktop Execution Log"));
        self.write_line(&format!("# Started: {}", Utc::now().to_rfc3339()));
        self.write_line(&format!("# Project ID: {}", self.project_id));
        self.write_line("");

        Ok(log_path)
    }

    /// Write a log entry
    pub fn write_entry(&mut self, iteration: u32, content: &str, is_stderr: bool) {
        let prefix = if is_stderr { "ERR" } else { "OUT" };
        let timestamp = Utc::now().format("%H:%M:%S").to_string();
        let line = format!("[#{}] {} [{}] {}", iteration, timestamp, prefix, content);
        self.write_line(&line);
    }

    /// Write a raw line
    fn write_line(&mut self, line: &str) {
        if let Some(ref mut writer) = self.log_file {
            let _ = writeln!(writer, "{}", line);
            let _ = writer.flush();
        }
    }

    /// End the log session
    pub fn end_session(&mut self, status: &str) {
        self.write_line("");
        self.write_line(&format!("# Ended: {}", Utc::now().to_rfc3339()));
        self.write_line(&format!("# Status: {}", status));

        if let Some(ref mut writer) = self.log_file {
            let _ = writer.flush();
        }
        self.log_file = None;
    }

    /// Get the current log path
    pub fn get_log_path(&self) -> Option<&PathBuf> {
        self.log_path.as_ref()
    }
}

/// Clean up old logs based on retention policy
pub fn cleanup_old_logs(project_id: &uuid::Uuid, retention_days: u32) -> Result<u32, String> {
    let project_dir = get_project_dir(project_id).map_err(|e| e.to_string())?;
    let logs_dir = project_dir.join("logs");

    if !logs_dir.exists() {
        return Ok(0);
    }

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let mut deleted_count = 0u32;

    let entries = fs::read_dir(&logs_dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if let Ok(modified) = metadata.modified() {
                let modified_time: chrono::DateTime<Utc> = modified.into();
                if modified_time < cutoff {
                    if fs::remove_file(entry.path()).is_ok() {
                        deleted_count += 1;
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

/// Clean up all project logs
pub fn cleanup_all_logs(retention_days: u32) -> Result<u32, String> {
    let data_dir = crate::storage::get_data_dir().map_err(|e| e.to_string())?;
    let projects_dir = data_dir.join("projects");

    if !projects_dir.exists() {
        return Ok(0);
    }

    let mut total_deleted = 0u32;

    let entries = fs::read_dir(&projects_dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(project_id) = uuid::Uuid::parse_str(name) {
                    if let Ok(count) = cleanup_old_logs(&project_id, retention_days) {
                        total_deleted += count;
                    }
                }
            }
        }
    }

    Ok(total_deleted)
}
