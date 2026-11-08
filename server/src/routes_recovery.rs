use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use tower_http::limit::RequestBodyLimitLayer;

use crate::{
    auth::mutation_headers,
    coordinator_recovery::RecoveryCommand,
    error::{AppError, Result},
    routes::AppState,
    source_recovery_types::{PollRequest, RecoveryPolicy},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PolicyRequest {
    expected_policy_version: String,
    enabled: bool,
    maintenance: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrollRequest {
    rotate: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryRequest {
    request_id: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sources/{id}/recovery", get(status).put(policy))
        .route("/sources/{id}/recovery/enroll", post(enroll))
        .route("/sources/{id}/recovery/controller", delete(revoke))
        .route("/sources/{id}/recovery/retry", post(retry))
        .route("/source-recovery/{id}/poll", post(poll))
        .layer(RequestBodyLimitLayer::new(65_536))
}

async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    state.authorize(&headers).await?;
    state
        .coordinator
        .recovery(RecoveryCommand::Status { source_id: id })
        .await
        .map(Json)
}

async fn policy(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: std::result::Result<Json<PolicyRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>> {
    mutation_headers(&headers)?;
    state.authorize(&headers).await?;
    let Json(value) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    state
        .coordinator
        .recovery(RecoveryCommand::Policy {
            source_id: id,
            expected: value.expected_policy_version,
            policy: RecoveryPolicy {
                enabled: value.enabled,
                maintenance: value.maintenance,
            },
        })
        .await
        .map(Json)
}

async fn enroll(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: std::result::Result<Json<EnrollRequest>, JsonRejection>,
) -> Result<Response> {
    mutation_headers(&headers)?;
    state.authorize(&headers).await?;
    let Json(value) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    let mut response = Json(
        state
            .coordinator
            .recovery(RecoveryCommand::Enroll {
                source_id: id,
                rotate: value.rotate,
            })
            .await?,
    )
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    Ok(response)
}

async fn revoke(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    mutation_headers(&headers)?;
    state.authorize(&headers).await?;
    state
        .coordinator
        .recovery(RecoveryCommand::Revoke { source_id: id })
        .await
        .map(Json)
}

async fn retry(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: std::result::Result<Json<RetryRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>> {
    mutation_headers(&headers)?;
    state.authorize(&headers).await?;
    let Json(value) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    state
        .coordinator
        .recovery(RecoveryCommand::Retry {
            source_id: id,
            request_id: value.request_id,
        })
        .await
        .map(Json)
}

async fn poll(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: std::result::Result<Json<PollRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>> {
    let token = bearer(&headers)?;
    let Json(request) = body.map_err(|error| AppError::Invalid(error.body_text()))?;
    state
        .coordinator
        .recovery(RecoveryCommand::Poll {
            source_id: id,
            token,
            request,
        })
        .await
        .map(Json)
}

fn bearer(headers: &HeaderMap) -> Result<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(AppError::Unauthorized)
}
