use std::time::Duration;

use live_production_media::runner::{PipelineOutcome, probe_plugins, run_pipeline_for};

#[test]
fn plugin_probe_reports_every_stage_a_element() -> anyhow::Result<()> {
    // Given
    let required = ["compositor", "audiomixer", "splitmuxsink"];

    // When
    let report = probe_plugins(&required)?;

    // Then
    assert_eq!(report.len(), required.len());
    assert!(
        report
            .iter()
            .all(|entry| required.contains(&entry.name.as_str()))
    );
    Ok(())
}

#[test]
fn finite_pipeline_reaches_eos_when_manually_drained() -> anyhow::Result<()> {
    // Given
    let description = "videotestsrc is-live=true ! fakesink sync=false";

    // When
    let outcome = run_pipeline_for(description, Duration::from_millis(100))?;

    // Then
    assert_eq!(outcome, PipelineOutcome::Finalized);
    Ok(())
}
