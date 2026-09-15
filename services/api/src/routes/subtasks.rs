//! The subtask drawer: outcome, TLS summary, handshake trace, evidence, and the
//! Previous/Next neighbours the header buttons jump to.

use axum::extract::{Path, State};
use axum::Json;
use pqcas_domain::models::{Subtask, SubtaskReport};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskDetail {
    #[serde(flatten)]
    pub subtask: Subtask,
    /// `P-256 × mldsa44` — the drawer title.
    pub title: String,
    pub report: Option<SubtaskReport>,
    pub previous_id: Option<Uuid>,
    pub next_id: Option<Uuid>,
    pub position: i64,
    pub total: i64,
}

pub async fn get_subtask(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SubtaskDetail>> {
    let subtask = sqlx::query_as::<_, Subtask>(
        r#"
        SELECT id, test_id, ordinal, group_name, sig_alg, exec_status, result_status,
               report_status, failed_stage, error_summary, started_at, finished_at
        FROM subtasks WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("subtask"))?;

    let report = sqlx::query_as::<_, SubtaskReport>(
        r#"
        SELECT subtask_id, adapter, tls_summary, messages, cert_chain, http,
               diagnostics, raw, evidence, duration_ms
        FROM subtask_reports WHERE subtask_id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(state.db())
    .await?;

    // Neighbours are by ordinal within the same test, so Previous/Next walks the matrix
    // left-to-right, top-to-bottom exactly as it is drawn.
    let previous_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM subtasks WHERE test_id = $1 AND ordinal < $2 ORDER BY ordinal DESC LIMIT 1",
    )
    .bind(subtask.test_id)
    .bind(subtask.ordinal)
    .fetch_optional(state.db())
    .await?;

    let next_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM subtasks WHERE test_id = $1 AND ordinal > $2 ORDER BY ordinal ASC LIMIT 1",
    )
    .bind(subtask.test_id)
    .bind(subtask.ordinal)
    .fetch_optional(state.db())
    .await?;

    let total = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subtasks WHERE test_id = $1")
        .bind(subtask.test_id)
        .fetch_one(state.db())
        .await?;

    let title = format!("{} × {}", subtask.group_name, subtask.sig_alg);
    let position = subtask.ordinal as i64 + 1;

    Ok(Json(SubtaskDetail {
        title,
        report,
        previous_id,
        next_id,
        position,
        total,
        subtask,
    }))
}
