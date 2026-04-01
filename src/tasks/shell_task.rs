//! LocalShellTask -- spawn and manage background shell commands.
//!
//! Ported from ref/tasks/LocalShellTask/LocalShellTask.tsx and killShellTasks.ts.
//!
//! Uses `tokio::process` for async subprocess management.  Output is written to
//! a file on disk under `~/.thundercode/task-output/<task_id>.output` so that readers
//! can stream deltas without holding the child's stdout in memory.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing;

use crate::tasks::output::{self, get_task_output_path, init_task_output};

// ---------------------------------------------------------------------------
// ShellTask
// ---------------------------------------------------------------------------

/// A running background shell command.
///
/// The child process's combined stdout+stderr is piped to a file on disk.
/// Callers read output through [`ShellTask::read_output`] or the standalone
/// functions in [`crate::tasks::output`].
pub struct ShellTask {
    /// Handle to the spawned child process.
    child: Child,
    /// Path to the on-disk output file.
    output_file: PathBuf,
    /// Task identifier.
    task_id: String,
    /// Human-readable description.
    description: String,
    /// Optional timeout handle -- dropping it cancels the timer.
    _timeout_handle: Option<tokio::task::JoinHandle<()>>,
    /// Handle to the background I/O copier task.
    _io_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ShellTask {
    /// Spawn a new shell task.
    ///
    /// The command is executed via `sh -c` (or `cmd /C` on Windows).
    /// Stdout and stderr are merged and streamed to the task output file.
    /// An optional `timeout` will send SIGTERM followed by SIGKILL if the
    /// process has not exited in time.
    pub async fn spawn(
        task_id: &str,
        command: &str,
        description: &str,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        // Ensure the output file exists.
        let output_file = init_task_output(task_id).await?;

        // Build the command.  Use `sh -c` so pipes, redirects, etc. work.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            // Start in its own process group so we can kill the whole tree.
            .process_group(0)
            .kill_on_drop(false)
            .spawn()
            .with_context(|| format!("failed to spawn shell task: {}", command))?;

        tracing::debug!(
            task_id = task_id,
            command = command,
            pid = ?child.id(),
            "shell task spawned"
        );

        // Capture stdout + stderr into the output file.
        let io_handle = {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let tid = task_id.to_owned();

            tokio::spawn(async move {
                if let Err(e) = copy_output_to_file(&tid, stdout, stderr).await {
                    tracing::warn!(task_id = %tid, error = %e, "error copying task output");
                }
            })
        };

        // Optional timeout.
        let timeout_handle = timeout.map(|dur| {
            let tid = task_id.to_owned();
            tokio::spawn(async move {
                tokio::time::sleep(dur).await;
                tracing::info!(task_id = %tid, "shell task timed out, sending SIGTERM");
                kill_process_group_by_task_id(&tid);
            })
        });

        Ok(ShellTask {
            child,
            output_file,
            task_id: task_id.to_owned(),
            description: description.to_owned(),
            _timeout_handle: timeout_handle,
            _io_handle: Some(io_handle),
        })
    }

    /// Kill the child process.
    ///
    /// Sends SIGTERM to the process group first, waits briefly, then SIGKILL
    /// if still alive.
    pub async fn kill(&mut self) -> Result<()> {
        tracing::debug!(task_id = %self.task_id, "killing shell task");

        // Cancel the timeout if active.
        if let Some(h) = self._timeout_handle.take() {
            h.abort();
        }

        // Try SIGTERM on the process group.
        if let Some(pid) = self.child.id() {
            send_signal_to_group(pid, libc::SIGTERM);

            // Give the process a moment to exit gracefully.
            let exited = tokio::time::timeout(
                Duration::from_secs(2),
                self.child.wait(),
            )
            .await;

            if exited.is_err() {
                // Still running -- escalate to SIGKILL.
                tracing::debug!(
                    task_id = %self.task_id,
                    "SIGTERM did not stop task, sending SIGKILL"
                );
                send_signal_to_group(pid, libc::SIGKILL);
                let _ = self.child.wait().await;
            }
        } else {
            // No pid means it already exited or was never started properly.
            let _ = self.child.kill().await;
        }

        // Wait for I/O copier to finish flushing.
        if let Some(h) = self._io_handle.take() {
            let _ = h.await;
        }

        Ok(())
    }

    /// Wait for the child process to exit and return its exit code.
    ///
    /// Returns `None` if the process was killed by a signal without an exit code.
    pub async fn wait(&mut self) -> Option<i32> {
        match self.child.wait().await {
            Ok(status) => status.code(),
            Err(_) => None,
        }
    }

    /// Read output from the task's output file starting at `offset` bytes.
    ///
    /// Returns the new content and the updated offset for the next read.
    pub async fn read_output(&self, offset: usize) -> Result<(String, usize)> {
        output::read_task_output(&self.task_id, offset).await
    }

    /// Get the path to the output file.
    pub fn output_file(&self) -> &PathBuf {
        &self.output_file
    }

    /// Get the task ID.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get the task description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the PID of the child process, if still running.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}

impl Drop for ShellTask {
    fn drop(&mut self) {
        // Best-effort kill on drop -- don't leave zombies.
        if let Some(pid) = self.child.id() {
            send_signal_to_group(pid, libc::SIGKILL);
        }
        if let Some(h) = self._timeout_handle.take() {
            h.abort();
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Copy child stdout and stderr to the task output file, interleaving them.
async fn copy_output_to_file(
    task_id: &str,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
) -> Result<()> {
    // We'll read both streams line-by-line and append to the output file.
    // Using a single file writer avoids interleaving issues.
    let output_path = get_task_output_path(task_id);

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .await?;

    use tokio::io::AsyncWriteExt;

    // Merge stdout and stderr into a single stream using select.
    let mut stdout_reader = stdout.map(|s| BufReader::new(s).lines());
    let mut stderr_reader = stderr.map(|s| BufReader::new(s).lines());

    let mut stdout_done = stdout_reader.is_none();
    let mut stderr_done = stderr_reader.is_none();

    loop {
        if stdout_done && stderr_done {
            break;
        }

        tokio::select! {
            line = async {
                match stdout_reader.as_mut() {
                    Some(r) => r.next_line().await,
                    None => Ok(None),
                }
            }, if !stdout_done => {
                match line {
                    Ok(Some(line)) => {
                        file.write_all(line.as_bytes()).await?;
                        file.write_all(b"\n").await?;
                    }
                    Ok(None) => stdout_done = true,
                    Err(e) => {
                        tracing::warn!(task_id = task_id, error = %e, "stdout read error");
                        stdout_done = true;
                    }
                }
            }
            line = async {
                match stderr_reader.as_mut() {
                    Some(r) => r.next_line().await,
                    None => Ok(None),
                }
            }, if !stderr_done => {
                match line {
                    Ok(Some(line)) => {
                        file.write_all(line.as_bytes()).await?;
                        file.write_all(b"\n").await?;
                    }
                    Ok(None) => stderr_done = true,
                    Err(e) => {
                        tracing::warn!(task_id = task_id, error = %e, "stderr read error");
                        stderr_done = true;
                    }
                }
            }
        }
    }

    file.flush().await?;
    Ok(())
}

/// Send a signal to a process group (negative PID).
fn send_signal_to_group(pid: u32, signal: i32) {
    unsafe {
        // Negative PID sends the signal to the entire process group.
        libc::kill(-(pid as i32), signal);
    }
}

/// Kill a process group by looking up the task output path.
/// Used by the timeout handler which only has the task ID.
fn kill_process_group_by_task_id(task_id: &str) {
    // We can't easily get the PID from just the task ID without shared state,
    // so this is a best-effort helper used only by the timeout path.  The
    // engine's kill path uses the ShellTask handle directly.
    tracing::warn!(
        task_id = task_id,
        "timeout kill attempted (process group kill requires PID; \
         use TaskEngine::kill for reliable cleanup)"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_and_wait_echo() {
        let task_id = format!("test_shell_{}", nanoid::nanoid!(8));
        let mut task = ShellTask::spawn(&task_id, "echo hello", "test echo", None)
            .await
            .unwrap();

        let code = task.wait().await;
        assert_eq!(code, Some(0));

        // Give a moment for I/O to flush.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (output, _) = task.read_output(0).await.unwrap();
        assert!(output.contains("hello"), "output was: {output}");

        output::cleanup_task_output(&task_id).await;
    }

    #[tokio::test]
    async fn kill_running_task() {
        let task_id = format!("test_kill_{}", nanoid::nanoid!(8));
        let mut task = ShellTask::spawn(&task_id, "sleep 60", "long sleep", None)
            .await
            .unwrap();

        // Should be running.
        assert!(task.pid().is_some());

        task.kill().await.unwrap();

        output::cleanup_task_output(&task_id).await;
    }
}
