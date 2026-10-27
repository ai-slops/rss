mod source_epoch_support;

use source_epoch_support::reconnect_evidence;

#[test]
fn reconnect_latches_generation_at_serialized_reset_for_video_and_audio() -> anyhow::Result<()> {
    let evidence = reconnect_evidence()?;
    assert!(evidence.queued_old_video);
    assert!(evidence.queued_old_audio);
    assert_eq!(
        (
            evidence.second_video_marker,
            evidence.second_video_generation
        ),
        (0xff00_00ff, 2)
    );
    assert!((820..=940).contains(&evidence.second_audio_marker));
    assert_eq!(evidence.second_audio_generation, 2);
    assert!(evidence.second_video_pts > evidence.first_video_pts);
    assert!(evidence.second_audio_pts > evidence.first_audio_pts);
    println!(
        "epoch_evidence queued_red_gen1={} queued_440hz_gen1={} video_pts={}=>{} audio_pts={}=>{} blue_gen={} audio880_gen={}",
        evidence.queued_old_video,
        evidence.queued_old_audio,
        evidence.first_video_pts,
        evidence.second_video_pts,
        evidence.first_audio_pts,
        evidence.second_audio_pts,
        evidence.second_video_generation,
        evidence.second_audio_generation
    );
    Ok(())
}
