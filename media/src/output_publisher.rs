use std::{
    io::{BufRead, BufReader},
    os::fd::{AsRawFd, RawFd},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::output::{DestinationUrl, OutputError};

pub struct Publisher {
    child: Child,
    stdin: Option<ChildStdin>,
    progress_bytes: Arc<AtomicU64>,
    progress_worker: Option<JoinHandle<()>>,
}

impl Publisher {
    pub fn spawn(destination: &DestinationUrl) -> Result<Self, OutputError> {
        let mut child = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-loglevel",
                "quiet",
                "-progress",
                "pipe:2",
                "-stats_period",
                "0.25",
                "-f",
                "flv",
                "-i",
                "pipe:0",
                "-c",
                "copy",
                "-rw_timeout",
                "3000000",
                "-f",
                "flv",
            ])
            .arg(destination.expose_to_publisher())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| OutputError::Pipeline)?;
        let stdin = child.stdin.take().ok_or(OutputError::Pipeline)?;
        let stderr = child.stderr.take().ok_or(OutputError::Pipeline)?;
        let progress_bytes = Arc::new(AtomicU64::new(0));
        let worker_progress = Arc::clone(&progress_bytes);
        let progress_worker = thread::Builder::new()
            .name("output-publisher-progress".to_owned())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(value) = line.strip_prefix("total_size=")
                        && let Ok(bytes) = value.parse::<u64>()
                    {
                        worker_progress.store(bytes, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|_| OutputError::OwnerStopped)?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            progress_bytes,
            progress_worker: Some(progress_worker),
        })
    }

    pub fn input_fd(&self) -> Result<RawFd, OutputError> {
        self.stdin
            .as_ref()
            .map(AsRawFd::as_raw_fd)
            .ok_or(OutputError::Pipeline)
    }

    #[must_use]
    pub fn progress_bytes(&self) -> u64 {
        self.progress_bytes.load(Ordering::Relaxed)
    }

    pub fn has_exited(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_some()
    }

    pub fn stop(&mut self, kill: bool) {
        drop(self.stdin.take());
        if kill {
            let _ = self.child.kill();
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
            }
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(worker) = self.progress_worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.stop(true);
    }
}
