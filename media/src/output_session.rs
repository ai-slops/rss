use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    output::{OutputError, RecordingProvenanceGap, RecordingProvenanceInterval},
    protocol::EngineClockMapping,
    recording_provenance::ProvenanceSnapshot,
};

const SESSION_FILE: &str = "session.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub schema_version: u32,
    pub session_id: String,
    #[serde(default, with = "crate::decimal_u64::option")]
    pub wall_start_ns: Option<u64>,
    #[serde(default, with = "crate::decimal_u64::option")]
    pub wall_end_ns: Option<u64>,
    pub engine_run_id: Option<String>,
    pub clock_anchor: Option<EngineClockMapping>,
    pub state: SessionState,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub provenance: Vec<RecordingProvenanceInterval>,
    #[serde(default)]
    pub provenance_gaps: Vec<RecordingProvenanceGap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Active,
    Complete,
    Failed,
    Recovered,
    Unknown,
}

pub struct SessionStart<'a> {
    pub root: &'a Path,
    pub engine_run_id: Option<String>,
    pub clock_anchor: Option<EngineClockMapping>,
    pub wall_start_ns: u64,
}

pub struct RecordingSession {
    pub directory: PathBuf,
    pub metadata: SessionMetadata,
}

pub struct SessionRead {
    pub metadata: SessionMetadata,
    pub issues: Vec<String>,
}

impl RecordingSession {
    pub fn begin(start: SessionStart<'_>) -> Result<Self, OutputError> {
        let session_id = Uuid::now_v7().to_string();
        let directory = start.root.join(&session_id);
        fs::create_dir_all(&directory).map_err(|_| OutputError::Pipeline)?;
        let metadata = SessionMetadata {
            schema_version: 1,
            session_id,
            wall_start_ns: Some(start.wall_start_ns),
            wall_end_ns: None,
            engine_run_id: start.engine_run_id,
            clock_anchor: start.clock_anchor,
            state: SessionState::Active,
            gaps: Vec::new(),
            provenance: Vec::new(),
            provenance_gaps: Vec::new(),
        };
        write_metadata(&directory, &metadata)?;
        Ok(Self {
            directory,
            metadata,
        })
    }

    pub fn finish(&mut self, state: SessionState, wall_end_ns: u64) -> Result<(), OutputError> {
        self.metadata.state = state;
        self.metadata.wall_end_ns = Some(wall_end_ns);
        write_metadata(&self.directory, &self.metadata)
    }

    pub fn update_provenance(&mut self, snapshot: ProvenanceSnapshot) -> Result<(), OutputError> {
        self.metadata.provenance = snapshot.intervals;
        self.metadata.provenance_gaps = snapshot.gaps;
        write_metadata(&self.directory, &self.metadata)
    }

    #[must_use]
    pub fn pattern(&self) -> PathBuf {
        self.directory.join("segment-%05d.mkv")
    }
}

pub fn read(directory: &Path) -> SessionRead {
    let session_id = directory
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("unknown-session")
        .to_owned();
    let path = directory.join(SESSION_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<SessionMetadata>(&bytes).map_or_else(
            |_| SessionRead {
                metadata: unknown_metadata(session_id.clone()),
                issues: vec!["session_metadata_corrupt".to_owned()],
            },
            |metadata| SessionRead {
                metadata,
                issues: Vec::new(),
            },
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SessionRead {
            metadata: unknown_metadata(session_id),
            issues: vec!["session_metadata_missing".to_owned()],
        },
        Err(_) => SessionRead {
            metadata: unknown_metadata(session_id),
            issues: vec!["session_metadata_unreadable".to_owned()],
        },
    }
}

pub fn write_metadata(directory: &Path, metadata: &SessionMetadata) -> Result<(), OutputError> {
    atomic_json(&directory.join(SESSION_FILE), metadata)
}

pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), OutputError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| OutputError::RecordingFinalization)?;
    let parent = path.parent().ok_or(OutputError::RecordingFinalization)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("manifest"),
        Uuid::now_v7().simple()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| OutputError::RecordingFinalization)?;
    file.write_all(&bytes)
        .map_err(|_| OutputError::RecordingFinalization)?;
    file.sync_all()
        .map_err(|_| OutputError::RecordingFinalization)?;
    fs::rename(&temporary, path).map_err(|_| OutputError::RecordingFinalization)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| OutputError::RecordingFinalization)
}

fn unknown_metadata(session_id: String) -> SessionMetadata {
    SessionMetadata {
        schema_version: 1,
        session_id,
        wall_start_ns: None,
        wall_end_ns: None,
        engine_run_id: None,
        clock_anchor: None,
        state: SessionState::Unknown,
        gaps: vec!["wall_time_unavailable".to_owned()],
        provenance: Vec::new(),
        provenance_gaps: Vec::new(),
    }
}
