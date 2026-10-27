use std::path::PathBuf;

use tokio::sync::broadcast;

use crate::{
    coordinator::{Owner, now_ns},
    coordinator_take::{media_revision, scene_placements},
    error::{AppError, Result},
    media_bridge::MediaBridge,
    model::{ActualState, AppliedScene, DesiredState, OutputStatus},
    snapshot::{
        self, AudioRouteSnapshot, OutputStatusSnapshot, PluginSnapshot, PreviewSnapshot,
        ProgramSnapshot, ProjectSnapshot, Snapshot, StorageSnapshot,
    },
    store::Store,
};

impl Owner {
    pub(crate) fn open(
        database: PathBuf,
        state_directory: PathBuf,
        media: MediaBridge,
        events: broadcast::Sender<()>,
    ) -> Result<Self> {
        let store = Store::open(&database).map_err(AppError::Internal)?;
        let project = store.load().map_err(AppError::Internal)?;
        let first = project
            .scenes
            .first()
            .cloned()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("project has no scene")))?;
        let revision = media_revision(&first);
        let applied = media.initialize_program(&revision, scene_placements(&first)?)?;
        let program = AppliedScene {
            scene: first.clone(),
            program_revision: applied.program_revision.unwrap_or(revision),
            engine_run_id: crate::model::Id(media.engine_run_id().to_owned()),
            applied_at_ns: applied.at_ns.value().to_string(),
        };
        let output_statuses = project
            .outputs
            .iter()
            .map(|output| OutputStatus {
                output_id: output.id.clone(),
                desired: DesiredState::Stopped,
                actual: ActualState::Failed,
                last_media_at_ns: None,
                error: Some(
                    "The native media engine has no adapter for this configured output.".into(),
                ),
                platform_status: "Platform status unknown".into(),
            })
            .collect();
        Ok(Self {
            store,
            media,
            project,
            preview: first,
            program,
            previous_program: None,
            output_statuses,
            recordings: Vec::new(),
            state_directory,
            events,
        })
    }

    pub(crate) fn snapshot(&self) -> Result<Snapshot> {
        let project = ProjectSnapshot {
            id: self.project.id.clone(),
            name: self.project.name.clone(),
            revision: self.project.revision.clone(),
            canvas: self.project.canvas.clone(),
            scenes: self.project.scenes.clone(),
            sources: self
                .project
                .sources
                .iter()
                .map(|source| snapshot::source_snapshot(source, &self.project.scenes))
                .collect(),
            audio_routes: self
                .project
                .audio_routes
                .iter()
                .map(|route| AudioRouteSnapshot {
                    track_id: route.track_id.clone(),
                    gain_db: route.gain_db,
                    muted: route.muted,
                    routed: route.routed,
                    input_peak_db: -60.0,
                    output_peak_db: -60.0,
                    clipping: false,
                })
                .collect(),
            holdback_ms: self.project.holdback_ms,
            outputs: self.project.outputs.clone(),
            assets: self.project.assets.clone(),
        };
        Ok(Snapshot {
            cursor: self.store.cursor().map_err(AppError::Internal)?,
            server_time: now_ns(),
            engine_run_id: self.media.engine_run_id().to_owned(),
            project,
            preview: PreviewSnapshot {
                scene_id: self.preview.id.clone(),
                revision: self.preview.revision.clone(),
                rendered_revision: None,
                visible_revision: None,
                media_state: "stale",
            },
            program: ProgramSnapshot {
                scene_id: self.program.scene.id.clone(),
                scene_revision: self.program.scene.revision.clone(),
                program_revision: self.program.program_revision.clone(),
                applied_at_ns: self.program.applied_at_ns.clone(),
                health: "warning",
                previous_scene_id: self
                    .previous_program
                    .as_ref()
                    .map(|value| value.scene.id.clone()),
            },
            output_statuses: self.output_statuses.iter().map(output_snapshot).collect(),
            recordings: self.recordings.clone(),
            plugins: Vec::<PluginSnapshot>::new(),
            storage: StorageSnapshot {
                free_bytes: "0".into(),
                total_bytes: "0".into(),
                writable: self.state_directory.is_dir(),
            },
        })
    }
}

fn output_snapshot(status: &OutputStatus) -> OutputStatusSnapshot {
    OutputStatusSnapshot {
        output_id: status.output_id.clone(),
        desired: match status.desired {
            DesiredState::Running => "running",
            DesiredState::Stopped => "stopped",
        },
        actual: match status.actual {
            ActualState::Stopped => "stopped",
            ActualState::Starting => "starting",
            ActualState::Running => "running",
            ActualState::Retrying => "retrying",
            ActualState::Finalizing => "finalizing",
            ActualState::Failed => "failed",
        },
        last_media_at_ns: status.last_media_at_ns.clone(),
        error: status.error.clone(),
        platform_status: "Platform status unknown",
    }
}
