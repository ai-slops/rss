use anyhow::{Context, Result};
use std::{fs, time::Duration};

#[derive(Clone, Copy)]
pub struct OwnerIdentity {
    pid: u32,
    start_ticks: u64,
}

pub fn from_environment() -> Result<Option<OwnerIdentity>> {
    match (
        std::env::var("LIVE_PRODUCTION_OWNER_PID"),
        std::env::var("LIVE_PRODUCTION_OWNER_START_TICKS"),
    ) {
        (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => {
            if std::env::var("LIVE_PRODUCTION_SYSTEM_SERVICE").as_deref() == Ok("true") {
                return Ok(None);
            }
            let own_stat =
                fs::read_to_string("/proc/self/stat").context("read application parent")?;
            let parent: u32 = own_stat
                .rsplit_once(") ")
                .context("invalid application identity")?
                .1
                .split_whitespace()
                .nth(1)
                .context("missing parent PID")?
                .parse()?;
            anyhow::ensure!(
                parent > 1,
                "Launch through scripts/serve.sh or an explicitly configured system service; an orphaned daemon cannot own production."
            );
            let stat = fs::read_to_string(format!("/proc/{parent}/stat"))
                .context("read launcher identity")?;
            Ok(Some(OwnerIdentity {
                pid: parent,
                start_ticks: start_ticks(&stat).context("missing launcher generation")?,
            }))
        }
        (Ok(pid), Ok(ticks)) => {
            let owner = OwnerIdentity {
                pid: pid.parse().context("invalid supervisor PID")?,
                start_ticks: ticks.parse().context("invalid supervisor generation")?,
            };
            anyhow::ensure!(
                owner.pid > 1 && matches_owner(owner),
                "The owning supervisor is no longer running."
            );
            Ok(Some(owner))
        }
        _ => anyhow::bail!("Supervision requires both an owner PID and its process generation."),
    }
}

fn matches_owner(owner: OwnerIdentity) -> bool {
    fs::read_to_string(format!("/proc/{}/stat", owner.pid))
        .ok()
        .and_then(|text| start_ticks(&text))
        .is_some_and(|ticks| ticks == owner.start_ticks)
}

fn start_ticks(stat: &str) -> Option<u64> {
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

pub async fn owner_gone(owner: Option<OwnerIdentity>) {
    let Some(owner) = owner else {
        return std::future::pending().await;
    };
    let mut interval = tokio::time::interval(Duration::from_millis(250));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if !matches_owner(owner) {
            tracing::warn!("supervising process disappeared; shutting down owned production");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_pid_with_different_generation_is_not_our_owner() {
        assert!(!matches_owner(OwnerIdentity {
            pid: std::process::id(),
            start_ticks: 0
        }));
    }

    #[test]
    fn process_generation_is_read_without_exposing_arguments() {
        let stat = fs::read_to_string("/proc/self/stat").expect("self identity");
        let ticks = start_ticks(&stat).expect("generation");
        assert!(matches_owner(OwnerIdentity {
            pid: std::process::id(),
            start_ticks: ticks
        }));
    }
}
