use crate::{
    command::{Action, Command, ScenePayload},
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    model::{
        ActualState, Content, Crop, DesiredState, Fit, Id, Output, OutputAdapter, OutputStatus,
        Rect, SceneItem, Source, SourceAdapter, Track, TrackKind,
    },
    scene_commands, validation,
};

impl Owner {
    pub(crate) fn add_scene_item(&mut self, command: &Command, source_id: &Id) -> Result<Applied> {
        let source = self
            .project
            .sources
            .iter()
            .find(|source| &source.id == source_id)
            .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
        let track = source
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .ok_or_else(|| AppError::Invalid("This source has no video track.".into()))?;
        let mut scene = self.scene(&command.target_id)?.clone();
        if command.expected_revision.is_some() {
            validation::revision(command.expected_revision.as_deref(), &scene.revision)?;
        }
        scene.items.push(SceneItem {
            id: Id::new(),
            name: source.name.clone(),
            visible: true,
            locked: false,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: f64::from(self.project.canvas.width),
                height: f64::from(self.project.canvas.height),
            },
            crop: Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            fit: Fit::Contain,
            opacity: 1.0,
            content: Content::Track {
                track_id: track.id.clone(),
            },
        });
        let save_command = Command {
            id: command.id.clone(),
            target_id: command.target_id.clone(),
            expected_revision: Some(scene.revision.clone()),
            expected_engine_run_id: command.expected_engine_run_id.clone(),
            action: Action::SceneItemAdd(crate::command::AddSceneItem {
                source_id: source_id.clone(),
            }),
        };
        let saved =
            scene_commands::save(&mut self.project, &save_command, &ScenePayload { scene })?;
        if self.preview.id == saved.id {
            self.preview = saved.clone();
        }
        Ok(Applied::revision(saved.revision))
    }

    pub(crate) fn create_source(&mut self, name: &str, adapter: &SourceAdapter) -> Result<Applied> {
        validation::name(name)?;
        let adapter = self.unique_adapter(adapter);
        let slot = (0..crate::media_bridge_pipeline::MEDIA_TRACK_SLOTS)
            .find(|slot| {
                let video = crate::media_bridge_pipeline::video_track_id(*slot);
                let audio = crate::media_bridge_pipeline::audio_track_id(*slot);
                self.project
                    .sources
                    .iter()
                    .flat_map(|source| &source.tracks)
                    .all(|track| track.id.0 != video && track.id.0 != audio)
            })
            .ok_or_else(|| {
                AppError::Unavailable(
                    "The native media profile supports eight source pairs.".into(),
                )
            })?;
        self.project.sources.push(Source {
            id: Id::new(),
            name: name.into(),
            enabled: true,
            adapter,
            tracks: vec![
                Track {
                    id: Id(crate::media_bridge_pipeline::video_track_id(slot)),
                    kind: TrackKind::Video,
                    correction_ms: 0,
                },
                Track {
                    id: Id(crate::media_bridge_pipeline::audio_track_id(slot)),
                    kind: TrackKind::Audio,
                    correction_ms: 0,
                },
            ],
            record_on_connect: false,
        });
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }

    pub(crate) fn delete_source(&mut self, command: &Command) -> Result<Applied> {
        let source = self
            .project
            .sources
            .iter()
            .find(|source| source.id == command.target_id)
            .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
        let referenced = source.tracks.iter().any(|track| self.project.scenes.iter()
            .flat_map(|scene| &scene.items)
            .any(|item| matches!(&item.content, Content::Track { track_id } if track_id == &track.id)));
        if referenced {
            return Err(AppError::Conflict(
                "Remove this source from every scene before deleting it.".into(),
            ));
        }
        self.project
            .sources
            .retain(|source| source.id != command.target_id);
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }

    pub(crate) fn create_output(&mut self, name: &str, adapter: &OutputAdapter) -> Result<Applied> {
        validation::name(name)?;
        let output = Output {
            id: Id::new(),
            name: name.into(),
            adapter: adapter.clone(),
        };
        self.output_statuses.push(OutputStatus {
            output_id: output.id.clone(),
            desired: DesiredState::Stopped,
            actual: ActualState::Failed,
            last_media_at_ns: None,
            error: Some(
                "The native media engine has no Program-connected adapter for this output.".into(),
            ),
            platform_status: "Platform status unknown".into(),
        });
        self.project.outputs.push(output);
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }

    pub(crate) fn delete_output(&mut self, command: &Command) -> Result<Applied> {
        let status = self
            .output_statuses
            .iter()
            .find(|status| status.output_id == command.target_id)
            .ok_or_else(|| AppError::Invalid("Output no longer exists.".into()))?;
        if status.desired == DesiredState::Running {
            return Err(AppError::Conflict(
                "Stop this output before deleting it.".into(),
            ));
        }
        self.project
            .outputs
            .retain(|output| output.id != command.target_id);
        self.output_statuses
            .retain(|status| status.output_id != command.target_id);
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }

    fn unique_adapter(&self, adapter: &SourceAdapter) -> SourceAdapter {
        match adapter {
            SourceAdapter::Synthetic {
                pattern,
                frequency_hz,
            } => SourceAdapter::Synthetic {
                pattern: pattern.clone(),
                frequency_hz: *frequency_hz,
            },
            SourceAdapter::Srt { port, buffer_ms } => {
                let mut candidate = *port;
                while self.project.sources.iter().any(|source| {
                    matches!(source.adapter, SourceAdapter::Srt { port, .. } if port == candidate)
                }) { candidate = candidate.saturating_add(1); }
                SourceAdapter::Srt {
                    port: candidate,
                    buffer_ms: *buffer_ms,
                }
            }
        }
    }
}
