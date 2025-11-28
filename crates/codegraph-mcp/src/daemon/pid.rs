// ABOUTME: PID file management for daemon lifecycle
// ABOUTME: Handles writing, reading, and cleaning up PID files

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// PID file manager for daemon processes
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Create a new PID file manager with the given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a PID file manager with the default path
    pub fn default_path(project_root: &Path) -> PathBuf {
        project_root.join(".codegraph").join("daemon.pid")
    }

    /// Write the current process ID to the PID file
    pub fn write(&self) -> Result<()> {
        let pid = std::process::id();

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create PID file directory: {:?}", parent))?;
        }

        fs::write(&self.path, pid.to_string())
            .with_context(|| format!("Failed to write PID file: {:?}", self.path))?;

        info!("PID file written: {:?} (PID: {})", self.path, pid);
        Ok(())
    }

    /// Read the PID from the file
    pub fn read(&self) -> Result<Option<u32>> {
        if !self.path.exists() {
            debug!("PID file does not exist: {:?}", self.path);
            return Ok(None);
        }

        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read PID file: {:?}", self.path))?;

        let pid: u32 = content
            .trim()
            .parse()
            .with_context(|| format!("Invalid PID in file: {:?}", self.path))?;

        Ok(Some(pid))
    }

    /// Remove the PID file
    pub fn remove(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .with_context(|| format!("Failed to remove PID file: {:?}", self.path))?;
            info!("PID file removed: {:?}", self.path);
        }
        Ok(())
    }

    /// Check if a process with the stored PID is running
    pub fn is_process_running(&self) -> Result<bool> {
        match self.read()? {
            Some(pid) => Ok(is_pid_running(pid)),
            None => Ok(false),
        }
    }

    /// Get the path to the PID file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Clean up stale PID file if process is not running
    pub fn cleanup_stale(&self) -> Result<bool> {
        if let Some(pid) = self.read()? {
            if !is_pid_running(pid) {
                warn!("Removing stale PID file for non-running process {}", pid);
                self.remove()?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Check if a process with the given PID is running
#[cfg(unix)]
fn is_pid_running(pid: u32) -> bool {
    // Use kill -0 via shell to check if process exists
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_pid_running(_pid: u32) -> bool {
    // On non-Unix systems, assume process is running if we can't check
    true
}

impl Drop for PidFile {
    fn drop(&mut self) {
        // Only remove PID file if it contains our PID
        if let Ok(Some(stored_pid)) = self.read() {
            if stored_pid == std::process::id() {
                let _ = self.remove();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pid_file_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("test.pid");
        let pid_file = PidFile::new(&pid_path);

        pid_file.write().unwrap();
        let read_pid = pid_file.read().unwrap();

        assert_eq!(read_pid, Some(std::process::id()));
    }

    #[test]
    fn test_pid_file_remove() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("test.pid");
        let pid_file = PidFile::new(&pid_path);

        pid_file.write().unwrap();
        assert!(pid_path.exists());

        pid_file.remove().unwrap();
        assert!(!pid_path.exists());
    }

    #[test]
    fn test_pid_file_nonexistent_read() {
        let pid_file = PidFile::new("/nonexistent/path/test.pid");
        let result = pid_file.read().unwrap();
        assert_eq!(result, None);
    }
}
