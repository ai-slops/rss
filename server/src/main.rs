use std::{
    fs,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::Context;
use live_production::{
    coordinator::Coordinator,
    media_bridge::MediaBridge,
    routes::{self, AppState},
};
use live_production_media::preview::PreviewOutputs;
use live_production_media::protocol::EngineRunId;
mod supervision;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let owner_watch = supervision::from_environment()?;
    let root = std::env::current_dir().context("resolve working directory")?;
    let state_directory = env_path("LIVE_PRODUCTION_STATE_DIR", root.join("var"));
    prepare_state_directory(&state_directory)?;
    let _state_lock = lock_state_directory(&state_directory)?;
    let password = operator_password(&state_directory)?;
    let media = MediaBridge::spawn().map_err(anyhow::Error::from)?;
    let engine_run_id = EngineRunId::new(media.engine_run_id()).map_err(anyhow::Error::from)?;
    let coordinator = Coordinator::spawn(state_directory.join("project.sqlite"), media)
        .map_err(anyhow::Error::from)?;
    let shutdown_coordinator = coordinator.clone();
    let secure_cookie = std::env::var("LIVE_PRODUCTION_SECURE_COOKIE")
        .unwrap_or_else(|_| "true".into())
        .parse::<bool>()
        .context("LIVE_PRODUCTION_SECURE_COOKIE must be true or false")?;
    let previews = PreviewOutputs::spawn(&engine_run_id).map_err(anyhow::Error::from)?;
    let media_state = live_production::routes_media::MediaState::new(previews);
    let state = AppState::with_media(&password, secure_cookie, coordinator, media_state);
    let web_dist = env_path("LIVE_PRODUCTION_WEB_DIST", root.join("web/dist"));
    let address: SocketAddr = std::env::var("LIVE_PRODUCTION_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()
        .context("parse bind address")?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context("bind HTTP listener")?;
    tracing::info!(%address, state_directory = %state_directory.display(), "live production daemon started");
    let serve_result = axum::serve(listener, routes::router(state, web_dist))
        .with_graceful_shutdown(shutdown_signal(owner_watch))
        .await
        .context("serve HTTP");
    let shutdown_result = shutdown_coordinator
        .shutdown()
        .await
        .map_err(anyhow::Error::from);
    serve_result?;
    shutdown_result?;
    Ok(())
}

fn env_path(name: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(name).map_or(default, PathBuf::from)
}

fn prepare_state_directory(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("create state directory {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("protect state directory {}", path.display()))?;
    }
    Ok(())
}

fn lock_state_directory(path: &Path) -> anyhow::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path.join("daemon.lock"))
        .context("open daemon ownership lock")?;
    file.try_lock().context("Another daemon owns this state directory. Stop that service or choose a separate LIVE_PRODUCTION_STATE_DIR.")?;
    Ok(file)
}

fn operator_password(state_directory: &Path) -> anyhow::Result<String> {
    if let Ok(password) = std::env::var("LIVE_PRODUCTION_PASSWORD") {
        validate_operator_password(&password)?;
        return Ok(password);
    }
    let path = state_directory.join("operator-password");
    match fs::read_to_string(&path) {
        Ok(password) => {
            let password = password.trim_end().to_owned();
            validate_operator_password(&password)?;
            return Ok(password);
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error)
                .with_context(|| format!("read operator password {}", path.display()));
        }
        Err(_) => {}
    }
    let password = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("create operator password {}", path.display()))?;
    file.write_all(password.as_bytes())
        .context("write operator password")?;
    file.write_all(b"\n").context("finish operator password")?;
    Ok(password)
}

fn validate_operator_password(password: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        password.len() >= 16,
        "Operator password must contain at least 16 bytes; use the generated credential or a strong password."
    );
    anyhow::ensure!(
        password.len() <= 1024,
        "Operator password exceeds the 1024-byte limit."
    );
    Ok(())
}

async fn shutdown_signal(owner_watch: Option<supervision::OwnerIdentity>) {
    let control_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = control_c => {}, _ = terminate => {}, _ = supervision::owner_gone(owner_watch) => {} }
}

#[cfg(test)]
mod tests {
    use super::{lock_state_directory, validate_operator_password};

    #[test]
    fn weak_operator_password_is_rejected() {
        assert!(validate_operator_password("short").is_err());
    }

    #[test]
    fn generated_strength_password_is_accepted() {
        assert!(validate_operator_password("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn one_state_directory_has_exactly_one_daemon_owner() {
        let directory = tempfile::tempdir().expect("state directory");
        let first = lock_state_directory(directory.path()).expect("first owner");
        assert!(lock_state_directory(directory.path()).is_err());
        drop(first);
        assert!(lock_state_directory(directory.path()).is_ok());
    }
}
