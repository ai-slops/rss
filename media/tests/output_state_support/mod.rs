use std::time::{Duration, Instant};

use live_production_media::output::{OutputId, OutputManager, OutputState};

pub fn wait_for_state(
    manager: &OutputManager,
    id: &OutputId,
    state: OutputState,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.actual == state {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!(
        "output did not reach {state:?}: {:?}",
        manager.snapshot(id)?
    )
}
