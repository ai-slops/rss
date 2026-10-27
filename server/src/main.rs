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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let root = std::env::current_dir().context("resolve working directory")?;
    let state_directory = env_path("LIVE_PRODUCTION_STATE_DIR", root.join("var"));
    prepare_state_directory(&state_directory)?;
    let password = operator_password(&state_directory)?;
    let media = MediaBridge::spawn().map_err(anyhow::Error::from)?;
    let coordinator = Coordinator::spawn(state_directory.join("project.sqlite"), media)
        .map_err(anyhow::Error::from)?;
    let state = AppState::new(&password, false, coordinator);
    let web_dist = env_path("LIVE_PRODUCTION_WEB_DIST", root.join("web/dist"));
    let address: SocketAddr = std::env::var("LIVE_PRODUCTION_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()
        .context("parse bind address")?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context("bind HTTP listener")?;
    tracing::info!(%address, state_directory = %state_directory.display(), "live production daemon started");
    axum::serve(listener, routes::router(state, web_dist))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve HTTP")?;
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

fn operator_password(state_directory: &Path) -> anyhow::Result<String> {
    if let Ok(password) = std::env::var("LIVE_PRODUCTION_PASSWORD") {
        return Ok(password);
    }
    let path = state_directory.join("operator-password");
    match fs::read_to_string(&path) {
        Ok(password) => return Ok(password.trim_end().to_owned()),
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

async fn shutdown_signal() {
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
    tokio::select! { _ = control_c => {}, _ = terminate => {} }
}
