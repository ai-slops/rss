use std::{fs, os::unix::fs::PermissionsExt, path::Path};

pub fn preserve(source: &Path, label: &str) -> anyhow::Result<()> {
    let Some(root) = std::env::var_os("LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR") else {
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let destination = root.join(label);
    fs::create_dir_all(
        destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("evidence parent"))?,
    )?;
    fs::create_dir(&destination)?;
    copy_directory(source, &destination)?;
    eprintln!("native_media_evidence={}", destination.display());
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir(&target)?;
            copy_directory(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), &target)?;
            fs::set_permissions(target, fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}
