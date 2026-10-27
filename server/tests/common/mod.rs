use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use live_production::{
    coordinator::Coordinator,
    media_bridge::MediaBridge,
    routes::{self, AppState},
};
use serde_json::{Value, json};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

pub struct Harness {
    pub app: axum::Router,
    pub _directory: Arc<TempDir>,
}

impl Harness {
    pub fn new() -> Self {
        let directory = Arc::new(tempfile::tempdir().expect("temporary state directory"));
        std::fs::create_dir(directory.path().join("dist")).expect("dist directory");
        std::fs::write(
            directory.path().join("dist/index.html"),
            "<main>live production</main>",
        )
        .expect("SPA index");
        let media = MediaBridge::spawn().expect("native media engine");
        let coordinator = Coordinator::spawn(directory.path().join("project.sqlite"), media)
            .expect("coordinator owner");
        let state = AppState::new("operator-password", false, coordinator);
        let app = routes::router(state, directory.path().join("dist"));
        Self {
            app,
            _directory: directory,
        }
    }

    pub async fn login(&self) -> String {
        let response = self
            .app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/auth/login",
                None,
                Some(json!({"password": "operator-password"})),
            ))
            .await
            .expect("login response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        response
            .headers()
            .get("set-cookie")
            .expect("session cookie")
            .to_str()
            .expect("cookie text")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned()
    }

    pub async fn snapshot(&self, cookie: &str) -> Value {
        json_response(
            self.app
                .clone()
                .oneshot(request("GET", "/api/v1/snapshot", Some(cookie), None))
                .await
                .expect("snapshot response"),
        )
        .await
    }

    pub async fn command(&self, cookie: &str, body: Value) -> Value {
        json_response(
            self.app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/commands",
                    Some(cookie),
                    Some(body),
                ))
                .await
                .expect("command response"),
        )
        .await
    }
}

pub fn request(
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-live-production", "1");
    if let Some(value) = cookie {
        builder = builder.header("cookie", value);
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .expect("request")
}

async fn json_response(response: axum::response::Response) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("JSON response")
}
