use std::process::Command;

use live_production_media::output::RecordingSegment;
use num_traits::ToPrimitive;

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaEvidence {
    pub nonblack_pixels: usize,
    pub pcm_rms: f64,
}

pub fn decode_recordings(segments: &[RecordingSegment]) -> anyhow::Result<MediaEvidence> {
    let mut evidence = MediaEvidence::default();
    for segment in segments {
        evidence.nonblack_pixels = evidence
            .nonblack_pixels
            .max(nonblack_pixels(&segment.path)?);
        evidence.pcm_rms = evidence.pcm_rms.max(pcm_rms(&segment.path)?);
        if evidence.nonblack_pixels > 500 && evidence.pcm_rms > 50.0 {
            return Ok(evidence);
        }
    }
    Ok(evidence)
}

fn nonblack_pixels(path: &std::path::Path) -> anyhow::Result<usize> {
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "video decode failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output
        .stdout
        .chunks_exact(3)
        .filter(|pixel| pixel.iter().copied().max().is_some_and(|value| value > 24))
        .count())
}

fn pcm_rms(path: &std::path::Path) -> anyhow::Result<f64> {
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-t",
            "1",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-f",
            "s16le",
            "pipe:1",
        ])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "audio decode failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let samples: Vec<_> = output
        .stdout
        .chunks_exact(2)
        .map(|bytes| f64::from(i16::from_le_bytes([bytes[0], bytes[1]])))
        .collect();
    let count = samples
        .len()
        .to_f64()
        .ok_or_else(|| anyhow::anyhow!("PCM sample count is not representable"))?;
    anyhow::ensure!(count > 0.0, "audio decode returned no samples");
    Ok((samples.iter().map(|sample| sample * sample).sum::<f64>() / count).sqrt())
}
