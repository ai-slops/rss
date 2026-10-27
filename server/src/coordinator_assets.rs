use crate::{
    asset::parse_image,
    command::{AssetUpload, Command},
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    model::{Asset, Id},
    validation,
};
use std::{fs, io::Write};

impl Owner {
    pub(crate) fn upload_asset(
        &mut self,
        command: &Command,
        payload: &AssetUpload,
    ) -> Result<Applied> {
        validation::revision(command.expected_revision.as_deref(), &self.project.revision)?;
        validation::name(&payload.name)?;
        let image = parse_image(&payload.data_base64, &payload.mime)?;
        let id = Id::new();
        let directory = self.state_directory.join("assets");
        fs::create_dir_all(&directory).map_err(|error| AppError::Internal(error.into()))?;
        let path = directory.join(format!("{}.{}", id.0, image.extension));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| AppError::Internal(error.into()))?;
        if let Err(error) = file.write_all(&image.bytes).and_then(|()| file.sync_all()) {
            if let Err(cleanup) = fs::remove_file(&path) {
                tracing::warn!(error = ?cleanup, "partial asset cleanup failed");
            }
            return Err(AppError::Internal(error.into()));
        }
        self.project.assets.push(Asset {
            id,
            version: "1".into(),
            name: payload.name.clone(),
            mime: image.mime.into(),
            path: path.to_string_lossy().into_owned(),
        });
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }
}
