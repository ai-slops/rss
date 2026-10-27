use crate::{
    command::Command,
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    media_bridge::AudioRequest,
    model::AudioRoute,
    validation,
};

impl Owner {
    pub(crate) fn apply_audio_routes(
        &mut self,
        command: &Command,
        routes: &[AudioRoute],
    ) -> Result<Applied> {
        if command.expected_revision.is_some() {
            validation::revision(command.expected_revision.as_deref(), &self.project.revision)?;
        }
        validation::routes(routes, &self.project)?;
        let applied = self.media.apply_audio(AudioRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            routes,
        })?;
        self.project.audio_routes = routes.to_vec();
        self.bump_project()?;
        Ok(Applied {
            revision: Some(self.project.revision.clone()),
            at_ns: applied.at_ns.value().to_string(),
        })
    }

    pub(crate) fn set_audio(
        &mut self,
        command: &Command,
        gain_db: Option<f64>,
        muted: Option<bool>,
    ) -> Result<Applied> {
        let mut routes = self.project.audio_routes.clone();
        let route = routes
            .iter_mut()
            .find(|route| route.track_id == command.target_id)
            .ok_or_else(|| {
                AppError::Invalid("Route this audio track before changing live parameters.".into())
            })?;
        if let Some(gain_db) = gain_db {
            route.gain_db = gain_db;
        }
        if let Some(muted) = muted {
            route.muted = muted;
        }
        validation::routes(&routes, &self.project)?;
        self.apply_audio_routes(command, &routes)
    }
}
