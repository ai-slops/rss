use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use live_production_media::output::{OutputError, OutputId, OutputManager};

pub fn observe_stop(outputs: &OutputManager, id: &OutputId) -> Result<(), OutputError> {
    let started = Instant::now();
    let done = AtomicBool::new(false);
    eprintln!(
        "drift_stop_enter wall_ns={} snapshot={:?}",
        wall_ns(),
        outputs.snapshot(id)
    );
    let (result, call_elapsed) = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::Acquire) {
                eprintln!(
                    "drift_stop_state wall_ns={} elapsed_ms={} snapshot={:?}",
                    wall_ns(),
                    started.elapsed().as_millis(),
                    outputs.snapshot(id)
                );
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let _completion = ObservationCompletion(&done);
        let call_started = Instant::now();
        let result = outputs.stop(id);
        (result, call_started.elapsed())
    });
    eprintln!(
        "drift_stop_return wall_ns={} stop_call_elapsed_ms={} observer_elapsed_ms={} result={:?} snapshot={:?}",
        wall_ns(),
        call_elapsed.as_millis(),
        started.elapsed().as_millis(),
        result,
        outputs.snapshot(id)
    );
    result
}

struct ObservationCompletion<'a>(&'a AtomicBool);

impl Drop for ObservationCompletion<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn wall_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_keeps_the_original_stop_error_and_terminates_its_monitor() {
        let outputs = OutputManager::new();
        let id = OutputId::new("diagnostic-missing").expect("output id");
        let started = Instant::now();
        assert!(matches!(
            observe_stop(&outputs, &id),
            Err(OutputError::UnknownOutput)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn observation_completion_releases_the_monitor_during_unwind() {
        let done = AtomicBool::new(false);
        let result = std::panic::catch_unwind(|| {
            let _completion = ObservationCompletion(&done);
            panic!("simulated diagnostic unwind");
        });
        assert!(result.is_err());
        assert!(done.load(Ordering::Acquire));
    }
}
