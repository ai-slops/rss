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
    let port = state.media.port(&stream)?;
    let permit = state
        .media
        .clients
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::Unavailable("The browser media viewer budget is full.".into()))?;
    Ok(upgrade.on_upgrade(move |browser| async move {
        let _permit = permit;
        if let Err(error) = relay(browser, port).await {
            tracing::warn!(%error, %stream, "browser media signalling ended");
        }
    }))
}

async fn relay(mut browser: axum::extract::ws::WebSocket, port: u16) -> anyhow::Result<()> {
    let endpoint = format!("ws://127.0.0.1:{port}");
    let (upstream, _) = tokio::time::timeout(Duration::from_secs(3), connect_async(endpoint))
        .await
        .map_err(|_| anyhow::anyhow!("signalling connection timed out"))??;
    let (mut upstream_write, mut upstream_read) = upstream.split();
    loop {
        tokio::select! {
            browser_message = browser.recv() => match browser_message {
                Some(Ok(message)) => upstream_write.send(to_upstream(message)).await?,
                Some(Err(error)) => return Err(error.into()),
                None => return Ok(()),
            },
            upstream_message = upstream_read.next() => match upstream_message {
                Some(Ok(message)) => browser.send(to_browser(message)?).await?,
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
