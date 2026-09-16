//! PQ-CAS test tasks: create (New Test), list (Tasks), read, matrix, live events.

use std::collections::HashSet;
use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::stream::Stream;
use pqcas_domain::jobs::{test_channel, ProbeJob};
use pqcas_domain::models::{Matrix, MatrixCell, Subtask, Test};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::queue;
use crate::routes::sse::subscribe;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTest {
    pub host: String,
    pub port: i32,
    #[serde(default = "default_path")]
    pub http_path: String,
    pub tool: String,
    pub plugin: String,
    pub groups: Vec<String>,
    pub sig_algs: Vec<String>,
}

fn default_path() -> String {
    "/".to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDetail {
    #[serde(flatten)]
    pub test: Test,
    pub subtasks: Vec<Subtask>,
}

/// Expand the selected axes into one subtask per (group, sig alg) pair and queue them.
///
/// The whole expansion happens in one transaction so a caller never sees a task with a
/// partial subtask set, and the jobs are only queued once the rows are committed —
/// otherwise a fast worker could look up a subtask that is not there yet.
pub async fn create_test(
    State(state): State<AppState>,
    Json(body): Json<NewTest>,
) -> ApiResult<Json<Test>> {
    if body.host.trim().is_empty() {
        return Err(ApiError::BadRequest("host is required".into()));
    }
    if !(1..=65535).contains(&body.port) {
        return Err(ApiError::BadRequest("port must be 1-65535".into()));
    }
    if body.groups.is_empty() || body.sig_algs.is_empty() {
        return Err(ApiError::BadRequest(
            "select at least one group and one signature algorithm".into(),
        ));
    }
    if !body.http_path.starts_with('/') {
        return Err(ApiError::BadRequest("http path must start with /".into()));
    }

    let groups = dedupe(&body.groups);
    let sig_algs = dedupe(&body.sig_algs);
    let subtask_count = groups.len() * sig_algs.len();
    let max = state.config().max_subtasks_per_test;
    if subtask_count > max {
        return Err(ApiError::BadRequest(format!(
            "{subtask_count} subtasks exceeds the limit of {max}; narrow the selection"
        )));
    }

    reject_unknown(&state, "kem_group", &groups).await?;
    reject_unknown(&state, "sig_alg", &sig_algs).await?;

    let mut tx = state.db().begin().await?;

    let test = sqlx::query_as::<_, Test>(
        r#"
        INSERT INTO tests (host, port, http_path, method, tool, plugin, status,
                           group_count, sig_alg_count, subtask_count)
        VALUES ($1, $2, $3, 'GET', $4, $5, 'pending', $6, $7, $8)
        RETURNING id, host, port, http_path, method, tool, plugin, status,
                  group_count, sig_alg_count, subtask_count, created_at, started_at, finished_at
        "#,
    )
    .bind(body.host.trim())
    .bind(body.port)
    .bind(&body.http_path)
    .bind(&body.tool)
    .bind(&body.plugin)
    .bind(groups.len() as i32)
    .bind(sig_algs.len() as i32)
    .bind(subtask_count as i32)
    .fetch_one(&mut *tx)
    .await?;

    // Row-major ordering is what the drawer's Previous/Next walks, and it matches how the
    // matrix reads on screen (a row per group, a column per signature algorithm).
    let mut ordinals = Vec::with_capacity(subtask_count);
    let mut group_names = Vec::with_capacity(subtask_count);
    let mut alg_names = Vec::with_capacity(subtask_count);
    for (g, group) in groups.iter().enumerate() {
        for (s, alg) in sig_algs.iter().enumerate() {
            ordinals.push((g * sig_algs.len() + s) as i32);
            group_names.push(group.clone());
            alg_names.push(alg.clone());
        }
    }

    // RETURNING does not promise to follow the SELECT's ORDER BY, so carry the identifying
    // columns back out and pair them up here instead of trusting row order.
    let inserted = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        INSERT INTO subtasks (test_id, ordinal, group_name, sig_alg)
        SELECT $1, o, g, a
        FROM UNNEST($2::int[], $3::text[], $4::text[]) AS t(o, g, a)
        RETURNING id, group_name, sig_alg
        "#,
    )
    .bind(test.id)
    .bind(&ordinals)
    .bind(&group_names)
    .bind(&alg_names)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let adapter = adapter_for(&body.tool, &body.plugin, &state.config().default_adapter);
    let jobs: Vec<ProbeJob> = inserted
        .into_iter()
        .map(|(subtask_id, group_name, sig_alg)| ProbeJob {
            test_id: test.id,
            subtask_id,
            host: test.host.clone(),
            port: test.port as u16,
            http_path: test.http_path.clone(),
            adapter: adapter.clone(),
            group_name,
            sig_alg,
            timeout_ms: state.config().probe_timeout_ms,
        })
        .collect();

    queue::enqueue_probes(&state, &jobs).await?;

    Ok(Json(test))
}

/// Tool/plugin is the lab's vocabulary for "which TLS stack"; map it to an adapter and
/// fall back to the configured default rather than failing a task creation.
fn adapter_for(tool: &str, plugin: &str, default: &str) -> String {
    match (tool, plugin) {
        (_, p) if p.starts_with("bq") => "boringssl".to_string(),
        (_, p) if p.starts_with("bo") => "boringssl".to_string(),
        (_, p) if p.starts_with("wo") => "wolfssl".to_string(),
        (_, p) if p.starts_with("go") => "go".to_string(),
        (_, p) if p.starts_with("open") => "openssl".to_string(),
        _ => default.to_string(),
    }
}

fn dedupe(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && seen.insert(v.clone()))
        .collect()
}

/// Everything selected must exist in the seeded catalog: a typo in a group name would
/// otherwise become a task full of failures that look like the target's fault.
async fn reject_unknown(state: &AppState, kind: &str, names: &[String]) -> ApiResult<()> {
    let known = sqlx::query_scalar::<_, String>(
        "SELECT name FROM algorithms WHERE kind = $1 AND name = ANY($2)",
    )
    .bind(kind)
    .bind(names)
    .fetch_all(state.db())
    .await?;

    let known: HashSet<String> = known.into_iter().collect();
    let unknown: Vec<&String> = names.iter().filter(|n| !known.contains(*n)).collect();
    if !unknown.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "unknown {kind}: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

pub async fn list_tests(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<Test>>> {
    let limit = query.limit.clamp(1, 200);
    let rows = sqlx::query_as::<_, Test>(
        r#"
        SELECT id, host, port, http_path, method, tool, plugin, status,
               group_count, sig_alg_count, subtask_count, created_at, started_at, finished_at
        FROM tests ORDER BY created_at DESC LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(state.db())
    .await?;
    Ok(Json(rows))
}

pub async fn get_test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TestDetail>> {
    let test = fetch_test(&state, id).await?;
    let subtasks = sqlx::query_as::<_, Subtask>(
        r#"
        SELECT id, test_id, ordinal, group_name, sig_alg, exec_status, result_status,
               report_status, failed_stage, error_summary, started_at, finished_at
        FROM subtasks WHERE test_id = $1 ORDER BY ordinal
        "#,
    )
    .bind(id)
    .fetch_all(state.db())
    .await?;
    Ok(Json(TestDetail { test, subtasks }))
}

pub async fn get_matrix(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Matrix>> {
    fetch_test(&state, id).await?;

    let rows = sqlx::query_as::<_, Subtask>(
        r#"
        SELECT id, test_id, ordinal, group_name, sig_alg, exec_status, result_status,
               report_status, failed_stage, error_summary, started_at, finished_at
        FROM subtasks WHERE test_id = $1 ORDER BY ordinal
        "#,
    )
    .bind(id)
    .fetch_all(state.db())
    .await?;

    // Axis order comes from the ordinal, which preserves what the student selected.
    let mut groups: Vec<String> = Vec::new();
    let mut sig_algs: Vec<String> = Vec::new();
    for row in &rows {
        if !groups.contains(&row.group_name) {
            groups.push(row.group_name.clone());
        }
        if !sig_algs.contains(&row.sig_alg) {
            sig_algs.push(row.sig_alg.clone());
        }
    }

    let cells = rows
        .into_iter()
        .map(|row| MatrixCell {
            subtask_id: row.id,
            group_name: row.group_name,
            sig_alg: row.sig_alg,
            result_status: row.result_status,
            exec_status: row.exec_status,
        })
        .collect();

    Ok(Json(Matrix {
        groups,
        sig_algs,
        cells,
    }))
}

pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    fetch_test(&state, id).await?;
    subscribe(&state, test_channel(id)).await
}

async fn fetch_test(state: &AppState, id: Uuid) -> ApiResult<Test> {
    sqlx::query_as::<_, Test>(
        r#"
        SELECT id, host, port, http_path, method, tool, plugin, status,
               group_count, sig_alg_count, subtask_count, created_at, started_at, finished_at
        FROM tests WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("test"))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn plugin_prefixes_pick_the_stack() {
        assert_eq!(adapter_for("pq-dsa", "bq2606", "openssl"), "boringssl");
        assert_eq!(adapter_for("pq-dsa", "open3x", "openssl"), "openssl");
        assert_eq!(adapter_for("pq-dsa", "mystery", "openssl"), "openssl");
    }

    #[test]
    fn duplicate_selections_collapse() {
        let input = vec![
            "mldsa65".into(),
            " mldsa65 ".into(),
            "mldsa44".into(),
            "".into(),
        ];
        assert_eq!(
            dedupe(&input),
            vec!["mldsa65".to_string(), "mldsa44".to_string()]
        );
    }
}
