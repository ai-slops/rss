use crate::output::OutputError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaTap {
    video_channel: String,
    audio_channel: String,
}

impl MediaTap {
    pub fn new(video_channel: &str, audio_channel: &str) -> Result<Self, OutputError> {
        validate_channel(video_channel)?;
        validate_channel(audio_channel)?;
        Ok(Self {
            video_channel: video_channel.to_owned(),
            audio_channel: audio_channel.to_owned(),
        })
    }

    #[must_use]
    pub fn video_channel(&self) -> &str {
        &self.video_channel
    }

    #[must_use]
    pub fn audio_channel(&self) -> &str {
        &self.audio_channel
    }
}

fn validate_channel(value: &str) -> Result<(), OutputError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    valid.then_some(()).ok_or(OutputError::InvalidTap)
}
