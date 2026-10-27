use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{Path, State, WebSocketUpgrade, ws::Message},
    http::HeaderMap,
    response::Response,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use live_production_media::preview::PreviewOutputs;
use tokio::sync::Semaphore;
use tokio_tungstenite::{connect_async, tungstenite};

use crate::{
    error::{AppError, Result},
    routes::AppState,
};

const MAXIMUM_MEDIA_CLIENTS: usize = 6;

struct MediaLease {
    media: MediaState,
    stream: String,
}
impl Drop for MediaLease {
    fn drop(&mut self) {
        if let Some(track) = self.stream.strip_prefix("source-")
            && let Some(outputs) = &self.media.outputs
        {
            outputs.release_source_audio(track);
        }
    }
}

#[derive(Clone)]
pub struct MediaState {
    outputs: Option<Arc<PreviewOutputs>>,
    clients: Arc<Semaphore>,
}

impl MediaState {
    #[must_use]
    pub fn new(outputs: PreviewOutputs) -> Self {
        Self {
            outputs: Some(Arc::new(outputs)),
            clients: Arc::new(Semaphore::new(MAXIMUM_MEDIA_CLIENTS)),
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            outputs: None,
            clients: Arc::new(Semaphore::new(0)),
        }
    }

    fn port(&self, stream: &str) -> Result<u16> {
        match stream {
            "preview" => self
                .outputs
                .as_ref()
                .map(|outputs| outputs.preview_port())
                .ok_or_else(|| {
                    AppError::Unavailable(
                        "Browser media is unavailable in this server mode.".into(),
                    )
                }),
            "program" => self
                .outputs
                .as_ref()
                .map(|outputs| outputs.program_port())
                .ok_or_else(|| {
                    AppError::Unavailable(
                        "Browser media is unavailable in this server mode.".into(),
                    )
                }),
            track if track.starts_with("source-") => self
                .outputs
                .as_ref()
                .ok_or_else(|| {
                    AppError::Unavailable(
                        "Browser media is unavailable in this server mode.".into(),
                    )
                })?
                .source_audio_port(track.trim_start_matches("source-"))
                .map_err(|error| AppError::Unavailable(error.to_string())),
            _ => Err(AppError::Invalid("Unknown media stream.".into())),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/media/{stream}/ws", get(media_socket))
}

async fn media_socket(
    State(state): State<AppState>,
    Path(stream): Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response> {
    state.authorize(&headers).await?;
    crate::media_signalling_policy::origin(
        &headers,
        std::env::var("LIVE_PRODUCTION_PUBLIC_ORIGIN")
            .ok()
            .as_deref(),
    )?;
    if let Some(track) = stream.strip_prefix("source-") {
        let snapshot = state.coordinator.snapshot().await?;
        if !snapshot
            .project
            .sources
            .iter()
            .flat_map(|source| &source.tracks)
            .any(|source| source.kind == crate::model::TrackKind::Audio && source.id.0 == track)
        {
            return Err(AppError::Invalid(
                "Select an existing source audio track to Listen.".into(),
            ));
        }
    }
    let permit = state
        .media
        .clients
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::Unavailable("The browser media viewer budget is full.".into()))?;
    let port = state.media.port(&stream)?;
    let lease = MediaLease {
        media: state.media.clone(),
        stream: stream.clone(),
    };
    Ok(upgrade
        .max_frame_size(65536)
        .max_message_size(131072)
        .on_upgrade(move |browser| async move {
            let _permit = permit;
            let _lease = lease;
            if let Err(error) = relay(browser, port, (state, headers)).await {
                tracing::warn!(%error, %stream, "browser media signalling ended");
            }
        }))
}

async fn relay(
    mut browser: axum::extract::ws::WebSocket,
    port: u16,
    authorization: (AppState, HeaderMap),
) -> anyhow::Result<()> {
    let endpoint = format!("ws://127.0.0.1:{port}");
    let (upstream, _) = tokio::time::timeout(Duration::from_secs(3), connect_async(endpoint))
        .await
        .map_err(|_| anyhow::anyhow!("signalling connection timed out"))??;
    let (mut upstream_write, mut upstream_read) = upstream.split();
    let mut budget = crate::media_signalling_policy::SessionBudget::default();
    let mut expiry = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = expiry.tick() => { if authorization.0.authorize(&authorization.1).await.is_err() { let _ = browser.close().await; return Ok(()); } },
            browser_message = browser.recv() => match browser_message {
                Some(Ok(message)) => {
                    if let Message::Text(text) = &message { budget.admit(text)?; }
                    if matches!(message, Message::Binary(_)) { anyhow::bail!("binary browser signalling is not supported"); }
                    upstream_write.send(to_upstream(message)).await?
                },
                Some(Err(error)) => return Err(error.into()),
                None => return Ok(()),
            },
            upstream_message = upstream_read.next() => match upstream_message {
                Some(Ok(message)) => { if let tungstenite::Message::Text(text) = &message { budget.observe(text); } browser.send(to_browser(message)?).await? },
                Some(Err(error)) => return Err(error.into()),
                None => return Ok(()),
            },
        }
    }
}

fn to_upstream(message: Message) -> tungstenite::Message {
    match message {
        Message::Text(text) => tungstenite::Message::Text(text.to_string().into()),
        Message::Binary(bytes) => tungstenite::Message::Binary(bytes),
        Message::Ping(bytes) => tungstenite::Message::Ping(bytes),
        Message::Pong(bytes) => tungstenite::Message::Pong(bytes),
        Message::Close(frame) => {
            tungstenite::Message::Close(frame.map(|value| tungstenite::protocol::CloseFrame {
                code: value.code.into(),
                reason: value.reason.to_string().into(),
            }))
        }
    }
}

fn to_browser(message: tungstenite::Message) -> anyhow::Result<Message> {
    match message {
        tungstenite::Message::Text(text) => Ok(Message::Text(text.to_string().into())),
        tungstenite::Message::Binary(bytes) => Ok(Message::Binary(bytes)),
        tungstenite::Message::Ping(bytes) => Ok(Message::Ping(bytes)),
        tungstenite::Message::Pong(bytes) => Ok(Message::Pong(bytes)),
        tungstenite::Message::Close(frame) => Ok(Message::Close(frame.map(|value| {
            axum::extract::ws::CloseFrame {
                code: value.code.into(),
                reason: value.reason.to_string().into(),
            }
        }))),
        tungstenite::Message::Frame(_) => Err(anyhow::anyhow!("unexpected raw websocket frame")),
    }
}
