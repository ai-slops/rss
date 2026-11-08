use std::{convert::Infallible, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};

use crate::{
    auth::{Auth, mutation_headers},
    command::Command,
    coordinator::Coordinator,
    error::{AppError, Result},
};

#[derive(Clone)]
pub struct AppState {
    auth: Arc<Mutex<Auth>>,
    pub(crate) coordinator: Coordinator,
    pub(crate) media: crate::routes_media::MediaState,
}

impl AppState {
    #[must_use]
    pub fn new(password: &str, secure_cookie: bool, coordinator: Coordinator) -> Self {
        Self {
            auth: Arc::new(Mutex::new(Auth::new(password, secure_cookie))),
            coordinator,
            media: crate::routes_media::MediaState::disabled(),
        }
    }

    #[must_use]
    pub fn with_media(
        password: &str,
        secure_cookie: bool,
        coordinator: Coordinator,
        media: crate::routes_media::MediaState,
    ) -> Self {
        Self {
            auth: Arc::new(Mutex::new(Auth::new(password, secure_cookie))),
            coordinator,
            media,
        }
    }

    pub(crate) async fn authorize(&self, headers: &HeaderMap) -> Result<()> {
        self.auth.lock().await.authorize(headers)
    }
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

#[derive(Deserialize)]
struct EventQuery {
    after: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DestinationProvisioning {
    destination_id: String,
    url: String,
}

pub fn router(state: AppState, web_dist: PathBuf) -> Router {
    let index = web_dist.join("index.html");
    let api = Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/snapshot", get(snapshot))
        .route("/commands", post(command))
        .layer(axum::extract::DefaultBodyLimit::max(6_000_000))
        .route("/output-destinations", post(provision_destination))
        .route(
            "/sources/{id}/publishing-endpoint",
            get(source_publishing_endpoint),
        )
        .route("/events", get(events))
        .merge(crate::routes_recovery::router())
        .merge(crate::routes_media::router())
        .layer(RequestBodyLimitLayer::new(6_000_000));
    Router::new()
        .nest("/api/v1", api)
        .fallback_service(ServeDir::new(web_dist).fallback(ServeFile::new(index)))
        .with_state(state)
}

async fn source_publishing_endpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    state.authorize(&headers).await?;
    let mut response =
        Json(state.coordinator.source_publishing_endpoint(id).await?).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    Ok(response)
}

async fn provision_destination(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: std::result::Result<Json<DestinationProvisioning>, JsonRejection>,
) -> Result<StatusCode> {
    mutation_headers(&headers)?;
    state.auth.lock().await.authorize(&headers)?;
    let Json(destination) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    state
        .coordinator
        .provision_destination(destination.destination_id, destination.url)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: std::result::Result<Json<Login>, JsonRejection>,
) -> Result<Response> {
    mutation_headers(&headers)?;
    let Json(login) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    let cookie = state.auth.lock().await.login(&login.password)?;
    let value = HeaderValue::from_str(&cookie).map_err(|error| AppError::Internal(error.into()))?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(header::SET_COOKIE, value);
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    mutation_headers(&headers)?;
    let mut auth = state.auth.lock().await;
    auth.authorize(&headers)?;
    auth.logout(&headers);
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("lp_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    );
    Ok(response)
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::snapshot::Snapshot>> {
    state.auth.lock().await.authorize(&headers)?;
    state.coordinator.snapshot().await.map(Json)
}

async fn command(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: std::result::Result<Json<Command>, JsonRejection>,
) -> Result<Json<serde_json::Value>> {
    mutation_headers(&headers)?;
    state.auth.lock().await.authorize(&headers)?;
    let Json(command) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    state.coordinator.execute(command).await.map(Json)
}

async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EventQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>>> {
    state.auth.lock().await.authorize(&headers)?;
    let after = query.after.unwrap_or_else(|| "0".into());
    let parsed_after = after
        .parse::<i64>()
        .map_err(|_| AppError::Invalid("Event cursor must be an integer.".into()))?;
    if parsed_after < 0 {
        return Err(AppError::Invalid(
            "Event cursor must be non-negative.".into(),
        ));
    }
    let mut notifications = state.coordinator.subscribe();
    let initial = state.coordinator.events(&after).await?;
    let coordinator = state.coordinator.clone();
    let (sender, receiver) = mpsc::channel(64);
    tokio::spawn(async move {
        let mut cursor = after;
        if send_page(&sender, &mut cursor, initial).await.is_err() {
            return;
        }
        loop {
            match notifications.recv().await {
                Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                    let Ok(page) = coordinator.events(&cursor).await else {
                        return;
                    };
                    if send_page(&sender, &mut cursor, page).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(receiver)).keep_alive(KeepAlive::default()))
}

async fn send_page(
    sender: &mpsc::Sender<std::result::Result<Event, Infallible>>,
    cursor: &mut String,
    page: crate::observation::EventPage,
) -> std::result::Result<(), ()> {
    if let Some(gap) = page.gap {
        let data = serde_json::to_string(&gap).map_err(|_| ())?;
        sender
            .send(Ok(Event::default().event("gap").data(data)))
            .await
            .map_err(|_| ())?;
    }
    for stored in page.events {
        cursor.clone_from(&stored.cursor);
        let data = serde_json::to_string(&stored).map_err(|_| ())?;
        sender
            .send(Ok(Event::default()
                .id(stored.cursor)
                .event(stored.kind)
                .data(data)))
            .await
            .map_err(|_| ())?;
    }
    Ok(())
}
