use axum::{
    extract::State,
    extract::rejection::JsonRejection,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};

#[derive(Serialize, Deserialize, Clone)]
struct DiskInfo {
    path: String,
    usage: f32,
    size: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct HostInfo {
    hostname: String,
    ip: String,
    uptime: f64,
    cpu_usage: f32,
    cpu_frequency: f32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    gpu_usage: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gpu_frequency: Option<f32>,

    cpu_temperature: f32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    gpu_temperature: Option<f32>,

    memory_usage: f32,
    memory_max: f32,
    disks: Vec<DiskInfo>,
    processes: Vec<String>,
    os_name: String,
    os_version: String,
    os_kernel: String,
    os_architecture: String,
    cpu_model: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    gpu_model: Option<String>,
}

#[derive(Clone, Default)]
struct AppState {
    hosts: Arc<Mutex<HashMap<String, HostInfo>>>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing (console output)
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let state = AppState::default();

    let app = Router::new()
        .route("/hosts", get(get_hosts).post(update_host))
        .with_state(state)
        .layer(TraceLayer::new_for_http()); // logs method, path, status, latency

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("Listening on {addr}");

    axum::serve(listener, app).await.unwrap();
}

async fn update_host(
    State(state): State<AppState>,
    payload: Result<Json<HostInfo>, JsonRejection>,
) -> Result<&'static str, (StatusCode, String)> {
    match payload {
        Ok(Json(host)) => {
            if let Ok(json_str) = serde_json::to_string_pretty(&host) {
                info!("Incoming /hosts POST:\n{}", json_str);
            }
            let mut hosts = state.hosts.lock().unwrap();
            hosts.insert(host.ip.clone(), host);
            Ok("ok")
        }
        Err(rej) => {
            info!("Invalid JSON: {rej}");
            Err((StatusCode::UNPROCESSABLE_ENTITY, rej.to_string()))
        }
    }
}

async fn get_hosts(State(state): State<AppState>) -> impl IntoResponse {
    info!("Incoming /hosts GET request");

    let hosts = state.hosts.lock().unwrap();
    Json(hosts.values().cloned().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::util::ServiceExt; // for `oneshot` and `ready`

    fn app() -> Router {
        let state = AppState::default();
        Router::new()
            .route("/hosts", get(get_hosts).post(update_host))
            .with_state(state)
    }

    #[tokio::test]
    async fn get_hosts_empty() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hosts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let hosts: Vec<HostInfo> = serde_json::from_slice(&body).unwrap();
        assert_eq!(hosts.len(), 0);
    }

    #[tokio::test]
    async fn post_host_ok() {
        let app = app();

        let host_info = HostInfo {
            hostname: "test-host".to_string(),
            ip: "127.0.0.1".to_string(),
            uptime: 123.45,
            cpu_usage: 50.0,
            cpu_frequency: 2.5,
            gpu_usage: None,
            gpu_frequency: None,
            cpu_temperature: 60.0,
            gpu_temperature: None,
            memory_usage: 4.0,
            memory_max: 16.0,
            disks: vec![],
            processes: vec![],
            os_name: "TestOS".to_string(),
            os_version: "1.0".to_string(),
            os_kernel: "6.0".to_string(),
            os_architecture: "x86_64".to_string(),
            cpu_model: "TestCPU".to_string(),
            gpu_model: None,
        };
        let host_info_json = serde_json::to_string(&host_info).unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/hosts")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(host_info_json))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hosts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let hosts: Vec<HostInfo> = serde_json::from_slice(&body).unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].hostname, "test-host");
    }

    #[tokio::test]
    async fn post_host_invalid_json() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/hosts")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("invalid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
