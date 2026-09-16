//! The three endpoints every adapter image exposes.

use std::sync::Arc;

use adapter_proto::{Capabilities, ProbeRequest, ProbeResponse};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::backend::{self, Backend};
use crate::Config;

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    backend: Backend,
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let backend = Backend::parse(&config.backend)?;
    let bind = config.bind.clone();
    let state = AppState {
        config: Arc::new(config),
        backend,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/capabilities", get(capabilities))
        .route("/probe", post(probe))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, ?backend, "adapter listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn capabilities(State(state): State<AppState>) -> Json<Capabilities> {
    Json(Capabilities {
        adapter: state.config.adapter_name(),
        version: state.config.version.clone(),
        kem_groups: state.config.kem_group_list(),
        sig_algs: state.config.sig_alg_list(),
        tls_versions: vec!["TLSv1.3".into(), "TLSv1.2".into()],
    })
}

async fn probe(
    State(state): State<AppState>,
    Json(request): Json<ProbeRequest>,
) -> Result<Json<ProbeResponse>, (StatusCode, Json<serde_json::Value>)> {
    if request.target.host.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "target host is required" })),
        ));
    }

    let response = backend::run(&state.config, state.backend, &request).await;
    Ok(Json(response))
}
