use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;

use crate::state::AppState;

/// Liveness: the process is up. Deliberately does not touch Postgres or Redis, so a slow
/// database never gets the container killed.
pub async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Readiness: dependencies answer. This is what the compose healthcheck and the deploy
/// workflow wait on.
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let db = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.db())
        .await
        .is_ok();

    let mut conn = state.redis();
    let redis = redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .is_ok();

    let ready = db && redis;
    let code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        code,
        Json(json!({ "ready": ready, "db": db, "redis": redis })),
    )
}

/// Minimal Prometheus exposition: queue depth and task counts are what actually matter
/// when a class of 30 students all hit "Create Task" at once.
pub async fn metrics(State(state): State<AppState>) -> (StatusCode, String) {
    let counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, count(*) FROM tests GROUP BY status",
    )
    .fetch_all(state.db())
    .await
    .unwrap_or_default();

    let queued =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subtasks WHERE exec_status = 'queued'")
            .fetch_one(state.db())
            .await
            .unwrap_or(0);

    let mut body = String::new();
    body.push_str("# HELP pqcas_tests_total Tests by status\n# TYPE pqcas_tests_total gauge\n");
    for (status, count) in counts {
        body.push_str(&format!(
            "pqcas_tests_total{{status=\"{status}\"}} {count}\n"
        ));
    }
    body.push_str("# HELP pqcas_subtasks_queued Subtasks waiting for a worker\n# TYPE pqcas_subtasks_queued gauge\n");
    body.push_str(&format!("pqcas_subtasks_queued {queued}\n"));
    (StatusCode::OK, body)
}
