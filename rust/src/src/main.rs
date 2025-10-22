use axum::{Router, routing::{get, post}, extract::State, Json};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};
use tokio::net::TcpListener;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, skip_serializing_if = "Option::is_none")]

struct HostInfo {
    hostname: String,
    ip: String,
    cpu_usage: f32,
    cpu_frequency: f32,
    gpu_usage: Option<f32>,
    gpu_frequency: Option<f32>,
    cpu_temperature: f32,
    gpu_temperature: Option<f32>,
    memory_usage: f32,
    memory_max: f32,
    disk_usage: f32,
    disk_size: f32,
    processes: Vec<String>,
    os_name: String,
    os_version: String,
    os_kernel: String,
    os_architecture: String,
    cpu_model: String,
    gpu_model: Option<String>,
}

#[derive(Clone, Default)]
struct AppState {
    hosts: Arc<Mutex<HashMap<String, HostInfo>>>,
}

#[tokio::main]
async fn main() {
    let state = AppState::default();

    let app = Router::new()
        .route("/hosts", get(get_hosts).post(update_host))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on {addr}");

    axum::serve(listener, app).await.unwrap();
}

async fn update_host(
    State(state): State<AppState>,
    Json(payload): Json<HostInfo>,
) -> &'static str {
    let mut hosts = state.hosts.lock().unwrap();
    hosts.insert(payload.ip.clone(), payload);
    "ok"
}

async fn get_hosts(State(state): State<AppState>) -> Json<Vec<HostInfo>> {
    let hosts = state.hosts.lock().unwrap();
    Json(hosts.values().cloned().collect())
}
