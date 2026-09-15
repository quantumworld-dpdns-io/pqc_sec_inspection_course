//! PQC KEM DEMO runs: library compatibility matrix, group handshake, priority probe.

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::stream::Stream;
use pqcas_domain::jobs::{kem_channel, KemJob};
use pqcas_domain::models::{Endpoint, KemMatrixCell, KemRun, KemRunLog};
use pqcas_domain::status::KemRunMode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::queue;
use crate::routes::sse::subscribe;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewKemRun {
    pub mode: KemRunMode,
    /// Endpoint slug from the TARGET ENDPOINT dropdown.
    pub endpoint: String,
    /// Selected `TLS_KEY_SHARE_GROUPS`; ignored by the priority mode, which decides its
    /// own offer lists as it narrows down the server's preference order.
    #[serde(default)]
    pub kem_groups: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KemRunDetail {
    #[serde(flatten)]
    pub run: KemRun,
    pub endpoint: Endpoint,
    pub logs: Vec<KemRunLog>,
    pub cells: Vec<KemMatrixCell>,
}

pub async fn create_run(
    State(state): State<AppState>,
    Json(body): Json<NewKemRun>,
) -> ApiResult<Json<KemRun>> {
    let endpoint = sqlx::query_as::<_, Endpoint>(
        "SELECT id, slug, label, label_zh, host, port, notes, enabled FROM endpoints WHERE slug = $1",
    )
    .bind(&body.endpoint)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("endpoint"))?;

    if !endpoint.enabled {
        return Err(ApiError::Conflict(format!(
            "endpoint {} is disabled",
            endpoint.slug
        )));
    }

    if body.mode == KemRunMode::GroupCompat && body.kem_groups.is_empty() {
        return Err(ApiError::BadRequest(
            "select at least one key_share group".into(),
        ));
    }

    let run = sqlx::query_as::<_, KemRun>(
        r#"
        INSERT INTO kem_runs (mode, endpoint_id, status, kem_groups)
        VALUES ($1, $2, 'pending', $3)
        RETURNING id, mode, endpoint_id, status, kem_groups, created_at, finished_at
        "#,
    )
    .bind(body.mode)
    .bind(endpoint.id)
    .bind(&body.kem_groups)
    .fetch_one(state.db())
    .await?;

    queue::enqueue_kem(
        &state,
        &KemJob {
            run_id: run.id,
            mode: run.mode,
            host: endpoint.host.clone(),
            port: endpoint.port as u16,
            kem_groups: body.kem_groups,
        },
    )
    .await?;

    Ok(Json(run))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    25
}

pub async fn list_runs(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<KemRun>>> {
    let rows = sqlx::query_as::<_, KemRun>(
        r#"
        SELECT id, mode, endpoint_id, status, kem_groups, created_at, finished_at
        FROM kem_runs ORDER BY created_at DESC LIMIT $1
        "#,
    )
    .bind(query.limit.clamp(1, 100))
    .fetch_all(state.db())
    .await?;
    Ok(Json(rows))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<KemRunDetail>> {
    let run = sqlx::query_as::<_, KemRun>(
        r#"
        SELECT id, mode, endpoint_id, status, kem_groups, created_at, finished_at
        FROM kem_runs WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("kem run"))?;

    let endpoint = sqlx::query_as::<_, Endpoint>(
        "SELECT id, slug, label, label_zh, host, port, notes, enabled FROM endpoints WHERE id = $1",
    )
    .bind(run.endpoint_id)
    .fetch_one(state.db())
    .await?;

    let logs = sqlx::query_as::<_, KemRunLog>(
        "SELECT run_id, seq, at, level, message FROM kem_run_logs WHERE run_id = $1 ORDER BY seq",
    )
    .bind(id)
    .fetch_all(state.db())
    .await?;

    let cells = sqlx::query_as::<_, KemMatrixCell>(
        "SELECT run_id, kem, adapter, verdict, detail FROM kem_matrix_cells WHERE run_id = $1 ORDER BY kem, adapter",
    )
    .bind(id)
    .fetch_all(state.db())
    .await?;

    Ok(Json(KemRunDetail { run, endpoint, logs, cells }))
}

pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT true FROM kem_runs WHERE id = $1")
        .bind(id)
        .fetch_optional(state.db())
        .await?;
    if exists.is_none() {
        return Err(ApiError::NotFound("kem run"));
    }
    subscribe(&state, kem_channel(id)).await
}
