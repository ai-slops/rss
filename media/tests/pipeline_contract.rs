use live_production_media::pipeline::{ProgramPipelineConfig, program_pipeline_description};
use live_production_media::protocol::Milliseconds;

#[test]
fn program_pipeline_contains_server_composition_mix_and_bounded_consumers() -> anyhow::Result<()> {
    // Given
    let config = ProgramPipelineConfig::synthetic(
        Milliseconds::new(500),
        Milliseconds::new(-250),
        Milliseconds::new(125),
        "/tmp/program-%05d.mkv",
    );

    // When
    let description = program_pipeline_description(&config)?;

    // Then
    assert!(description.contains("compositor name=program_compositor"));
    assert!(description.contains("compositor name=preview_compositor"));
    assert!(description.contains("audiomixer name=program_mixer"));
    assert!(description.contains("splitmuxsink name=program_recorder"));
    assert!(description.contains("queue max-size-buffers="));
    assert!(description.contains("leaky=downstream"));
    assert!(description.contains("identity name=stalled_preview sleep-time=500000"));
    Ok(())
}

#[test]
fn program_pipeline_applies_distinct_signed_track_offsets() -> anyhow::Result<()> {
    // Given
    let config = ProgramPipelineConfig::synthetic(
        Milliseconds::new(500),
        Milliseconds::new(-250),
        Milliseconds::new(125),
        "/tmp/program-%05d.mkv",
    );

    // When
    let description = program_pipeline_description(&config)?;

    // Then
    assert!(
        description
            .contains("video_timing_source is-live=true pattern=smpte timestamp-offset=250000000")
    );
    assert!(description.contains(
        "audio_timing_source is-live=true wave=sine freq=440 timestamp-offset=625000000"
    ));
    Ok(())
}
