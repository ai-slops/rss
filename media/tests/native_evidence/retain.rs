use std::path::{Path, PathBuf};

pub struct RetainOnDrop {
    source: PathBuf,
    label: &'static str,
}

impl RetainOnDrop {
    pub fn new(source: &Path, label: &'static str) -> Self {
        Self {
            source: source.to_path_buf(),
            label,
        }
    }
}

impl Drop for RetainOnDrop {
    fn drop(&mut self) {
        if let Err(error) = super::native_evidence::preserve(&self.source, self.label) {
            eprintln!("native_media_preservation_failed={error:#}");
        }
    }
}
