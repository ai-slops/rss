use std::{convert::Infallible, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Query, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
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
    coordinator: Coordinator,
}

impl AppState {
    #[must_use]
    pub fn new(password: &str, secure_cookie: bool, coordinator: Coordinator) -> Self {
        Self {
            auth: Arc::new(Mutex::new(Auth::new(password, secure_cookie))),
            coordinator,
        }
    }
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

#[derive(Deserialize)]
struct EventQuery {
    #[allow(dead_code)]
    after: Option<String>,
}

pub fn router(state: AppState, web_dist: PathBuf) -> Router {
    let index = web_dist.join("index.html");
    let api = Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/snapshot", get(snapshot))
        .route("/commands", post(command))
        .route("/events", get(events))
        .layer(RequestBodyLimitLayer::new(1_048_576));
    Router::new()
        .nest("/api/v1", api)
        .fallback_service(ServeDir::new(web_dist).fallback(ServeFile::new(index)))
        .with_state(state)
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
    Query(_query): Query<EventQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>>> {
    state.auth.lock().await.authorize(&headers)?;
    let initial = tokio_stream::once(Ok(Event::default().data("{}")));
    let updates = BroadcastStream::new(state.coordinator.subscribe()).map(|event| match event {
        Ok(()) => Ok(Event::default().data("{}")),
        Err(_) => Ok(Event::default().event("gap").data("{}")),
    });
    Ok(Sse::new(initial.chain(updates)).keep_alive(KeepAlive::default()))
}
