use crate::error::Result;
use anyhow::Context;
use dashmap::DashMap;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub transport: String,
    pub config_path: Option<PathBuf>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub status: ProcessStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

pub struct ProcessManager {
    processes: Arc<DashMap<String, ProcessInfo>>,
    pid_files: Arc<RwLock<Vec<PathBuf>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(DashMap::new()),
            pid_files: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start_stdio_server(
        &self,
        config: Option<PathBuf>,
        daemon: bool,
        pid_file: Option<PathBuf>,
        buffer_size: usize,
    ) -> Result<u32> {
        info!("Starting STDIO MCP server");

        let mut cmd = Command::new(std::env::current_exe()?);
        cmd.arg("serve-stdio");

        if let Some(config_path) = &config {
            cmd.arg("--config").arg(config_path);
        }

        cmd.arg("--buffer-size").arg(buffer_size.to_string());

        let child = if daemon {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("Failed to spawn STDIO server in daemon mode")?
        } else {
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .context("Failed to spawn STDIO server")?
        };

        let pid = child.id();

        // Store process info
        let info = ProcessInfo {
            pid,
            transport: "stdio".to_string(),
            config_path: config,
            start_time: chrono::Utc::now(),
            status: ProcessStatus::Running,
        };

        self.processes.insert(format!("stdio_{}", pid), info);

        // Write PID file if requested
        if let Some(pid_file_path) = pid_file {
            self.write_pid_file(&pid_file_path, pid).await?;
        }

        info!("STDIO server started with PID: {}", pid);
        Ok(pid)
    }

    pub async fn start_http_server(
        &self,
        host: String,
        port: u16,
        config: Option<PathBuf>,
        daemon: bool,
        pid_file: Option<PathBuf>,
        tls: bool,
        cert: Option<PathBuf>,
        key: Option<PathBuf>,
    ) -> Result<u32> {
        info!("Starting HTTP MCP server at {}:{}", host, port);

        let mut cmd = Command::new(std::env::current_exe()?);
        cmd.arg("serve-http")
            .arg("--host")
            .arg(&host)
            .arg("--port")
            .arg(port.to_string());

        if let Some(config_path) = &config {
            cmd.arg("--config").arg(config_path);
        }

        if tls {
            cmd.arg("--tls");
            if let Some(cert_path) = cert {
                cmd.arg("--cert").arg(cert_path);
            }
            if let Some(key_path) = key {
                cmd.arg("--key").arg(key_path);
            }
        }

        let child = if daemon {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("Failed to spawn HTTP server in daemon mode")?
        } else {
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .context("Failed to spawn HTTP server")?
        };

        let pid = child.id();

        // Store process info
        let info = ProcessInfo {
            pid,
            transport: format!("http://{}:{}", host, port),
            config_path: config,
            start_time: chrono::Utc::now(),
            status: ProcessStatus::Running,
        };

        self.processes.insert(format!("http_{}", pid), info);

        // Write PID file if requested
        if let Some(pid_file_path) = pid_file {
            self.write_pid_file(&pid_file_path, pid).await?;
        }

        info!("HTTP server started with PID: {}", pid);
        Ok(pid)
    }

    pub async fn start_dual_transport(
        &self,
        host: String,
        port: u16,
        buffer_size: usize,
        config: Option<PathBuf>,
        daemon: bool,
        pid_file: Option<PathBuf>,
    ) -> Result<(u32, u32)> {
        info!("Starting dual transport MCP servers");

        // Start STDIO server
        let stdio_pid = self
            .start_stdio_server(
                config.clone(),
                daemon,
                pid_file.as_ref().map(|p| {
                    let mut stdio_pid = p.clone();
                    stdio_pid.set_extension("stdio.pid");
                    stdio_pid
                }),
                buffer_size,
            )
            .await?;

        // Start HTTP server
        let http_pid = self
            .start_http_server(
                host,
                port,
                config,
                daemon,
                pid_file.as_ref().map(|p| {
                    let mut http_pid = p.clone();
                    http_pid.set_extension("http.pid");
                    http_pid
                }),
                false,
                None,
                None,
            )
            .await?;

        Ok((stdio_pid, http_pid))
    }

    pub async fn stop_server(&self, pid_file: Option<PathBuf>, force: bool) -> Result<()> {
        let pid = if let Some(pid_file_path) = pid_file {
            self.read_pid_file(&pid_file_path).await?
        } else {
            // Try to find running server
            self.find_running_server_pid()?
        };

        info!("Stopping server with PID: {}", pid);

        if force {
            self.force_kill(pid)?;
        } else {
            self.graceful_shutdown(pid)?;
        }

        // Update process status
        for mut entry in self.processes.iter_mut() {
            if entry.value().pid == pid {
                entry.status = ProcessStatus::Stopped;
            }
        }

        Ok(())
    }

    pub async fn get_status(&self, pid_file: Option<PathBuf>) -> Result<ProcessInfo> {
        let pid = if let Some(pid_file_path) = pid_file {
            self.read_pid_file(&pid_file_path).await?
        } else {
            self.find_running_server_pid()?
        };

        // Check if process is actually running
        if !self.is_process_running(pid)? {
            return Err(anyhow::anyhow!("Server is not running").into());
        }

        // Find process info
        for entry in self.processes.iter() {
            if entry.value().pid == pid {
                return Ok(entry.value().clone());
            }
        }

        Err(anyhow::anyhow!("Process information not found").into())
    }

    async fn write_pid_file(&self, path: &Path, pid: u32) -> Result<()> {
        fs::write(path, pid.to_string()).context("Failed to write PID file")?;

        let mut pid_files = self.pid_files.write().await;
        pid_files.push(path.to_path_buf());

        debug!("Wrote PID {} to {:?}", pid, path);
        Ok(())
    }

    async fn read_pid_file(&self, path: &Path) -> Result<u32> {
        let content = fs::read_to_string(path).context("Failed to read PID file")?;
        let pid = content
            .trim()
            .parse::<u32>()
            .context("Invalid PID in file")?;
        Ok(pid)
    }

    fn find_running_server_pid(&self) -> Result<u32> {
        for entry in self.processes.iter() {
            if entry.value().status == ProcessStatus::Running {
                return Ok(entry.value().pid);
            }
        }
        Err(anyhow::anyhow!("No running server found").into())
    }

    fn is_process_running(&self, pid: u32) -> Result<bool> {
        match signal::kill(Pid::from_raw(pid as i32), None) {
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::ESRCH) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Failed to check process: {}", e).into()),
        }
    }

    fn graceful_shutdown(&self, pid: u32) -> Result<()> {
        info!("Sending SIGTERM to process {}", pid);
        signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
            .context("Failed to send SIGTERM")?;

        // Wait for process to terminate
        std::thread::sleep(std::time::Duration::from_secs(5));

        if self.is_process_running(pid)? {
            warn!(
                "Process {} did not terminate gracefully, sending SIGKILL",
                pid
            );
            self.force_kill(pid)?;
        }

        Ok(())
    }

    fn force_kill(&self, pid: u32) -> Result<()> {
        info!("Sending SIGKILL to process {}", pid);
        signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
            .context("Failed to send SIGKILL")?;
        Ok(())
    }

    pub async fn cleanup(&self) -> Result<()> {
        let pid_files = self.pid_files.read().await;
        for pid_file in pid_files.iter() {
            if pid_file.exists() {
                fs::remove_file(pid_file).context("Failed to remove PID file")?;
                debug!("Removed PID file: {:?}", pid_file);
            }
        }
        Ok(())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_pid_file_operations() {
        let manager = ProcessManager::new();
        let temp_dir = tempdir().unwrap();
        let pid_file = temp_dir.path().join("test.pid");

        // Write PID file
        manager.write_pid_file(&pid_file, 12345).await.unwrap();
        assert!(pid_file.exists());

        // Read PID file
        let pid = manager.read_pid_file(&pid_file).await.unwrap();
        assert_eq!(pid, 12345);

        // Cleanup
        manager.cleanup().await.unwrap();
    }

    #[test]
    fn test_process_status() {
        let status = ProcessStatus::Running;
        assert_eq!(status, ProcessStatus::Running);

        let failed = ProcessStatus::Failed("Error".to_string());
        assert_ne!(failed, ProcessStatus::Running);
    }
}
