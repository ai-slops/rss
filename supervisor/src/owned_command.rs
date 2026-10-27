use crate::process_tree;
use anyhow::{Context, Result};
use nix::{
    errno::Errno,
    sys::{
        prctl::set_child_subreaper,
        signal::{Signal, kill, killpg},
        wait::{WaitPidFlag, waitpid},
    },
    unistd::{Pid, getpid, getppid},
};
use std::{
    collections::HashSet,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub struct OwnedCommand {
    child: Child,
    group: Pid,
    grace: Duration,
    cleaned: bool,
    parent: Option<process_tree::Identity>,
}

impl OwnedCommand {
    pub fn spawn(mut command: Command, grace: Duration) -> Result<Self> {
        set_child_subreaper(true).context("become a child subreaper")?;
        let owner = process_tree::identity(getpid()).context("read supervisor identity")?;
        let parent = (getppid().as_raw() > 1)
            .then(|| process_tree::identity(getppid()))
            .transpose()?;
        command
            .process_group(0)
            .env("LIVE_PRODUCTION_OWNER_PID", owner.pid.to_string())
            .env(
                "LIVE_PRODUCTION_OWNER_START_TICKS",
                owner.start_ticks.to_string(),
            );
        if std::env::var("LD_PRELOAD")
            .is_ok_and(|value| value == "/lib/libgcompat.so.0" || value == "libgcompat.so.0")
        {
            command.env_remove("LD_PRELOAD");
        }
        let child = command.spawn().context("start supervised command")?;
        let group =
            Pid::from_raw(i32::try_from(child.id()).context("child PID exceeds supported range")?);
        Ok(Self {
            child,
            group,
            grace,
            cleaned: false,
            parent,
        })
    }

    pub fn wait(&mut self, signal: &Arc<AtomicUsize>, timeout: Option<Duration>) -> Result<i32> {
        let started = Instant::now();
        let result = loop {
            if self
                .parent
                .as_ref()
                .is_some_and(|parent| !process_tree::still_same(parent))
            {
                break 125;
            }
            if let Some(status) = self
                .child
                .try_wait()
                .context("observe supervised command")?
            {
                break status
                    .code()
                    .unwrap_or_else(|| 128 + status.signal().unwrap_or(1));
            }
            let received = signal.load(Ordering::Relaxed);
            if received != 0 {
                break 128 + i32::try_from(received).unwrap_or(1);
            }
            if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                break 124;
            }
            thread::sleep(Duration::from_millis(20));
        };
        self.drain()?;
        Ok(result)
    }

    fn drain(&mut self) -> Result<()> {
        if self.cleaned {
            return Ok(());
        }
        // A private process group covers ordinary children; subreaping also owns setsid/double-fork descendants.
        let _ = killpg(self.group, Signal::SIGTERM);
        let started = Instant::now();
        let mut signalled = HashSet::new();
        loop {
            let _ = self.child.try_wait();
            let children =
                process_tree::descendants(getpid()).context("enumerate owned process tree")?;
            for process in &children {
                let forced = started.elapsed() >= self.grace;
                if process_tree::still_same(process)
                    && (forced || signalled.insert((process.pid, process.start_ticks)))
                {
                    let _ = kill(
                        process.pid,
                        if forced {
                            Signal::SIGKILL
                        } else {
                            Signal::SIGTERM
                        },
                    );
                }
                if process.pid != self.group {
                    match waitpid(process.pid, Some(WaitPidFlag::WNOHANG)) {
                        Ok(_) | Err(Errno::ECHILD | Errno::ESRCH) => {}
                        Err(error) => return Err(error).context("reap owned descendant"),
                    }
                }
            }
            if children.is_empty() {
                self.cleaned = true;
                return Ok(());
            }
            anyhow::ensure!(
                started.elapsed() < self.grace + Duration::from_secs(5),
                "owned processes did not exit after TERM/KILL cleanup"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for OwnedCommand {
    fn drop(&mut self) {
        if let Err(error) = self.drain() {
            eprintln!("owned process cleanup failed: {error:#}");
        }
    }
}
