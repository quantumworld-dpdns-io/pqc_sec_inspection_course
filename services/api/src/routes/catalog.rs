//! The seeded catalog: algorithms (checkbox grids, matrix axes) and lab endpoints
//! (Environment page, KEM DEMO target dropdown).

use axum::extract::{Query, State};
use axum::Json;
use pqcas_domain::models::{Algorithm, Endpoint};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlgorithmQuery {
    /// `kem_group` or `sig_alg`; omit for both.
    pub kind: Option<String>,
    /// Include algorithms the lab has switched off (rendered greyed out).
    #[serde(default = "default_true")]
    pub include_disabled: bool,
}

fn default_true() -> bool {
    true
}

pub async fn list_algorithms(
    State(state): State<AppState>,
    Query(query): Query<AlgorithmQuery>,
) -> ApiResult<Json<Vec<Algorithm>>> {
    if let Some(kind) = &query.kind {
        if kind != "kem_group" && kind != "sig_alg" {
            return Err(ApiError::BadRequest(format!("unknown algorithm kind {kind}")));
        }
    }

    let rows = sqlx::query_as::<_, Algorithm>(
        r#"
        SELECT id, kind, name, display_name, family, enabled, sort_order
        FROM algorithms
        WHERE ($1::text IS NULL OR kind = $1)
          AND ($2::bool OR enabled)
        ORDER BY kind, sort_order, name
        "#,
    )
    .bind(&query.kind)
    .bind(query.include_disabled)
    .fetch_all(state.db())
    .await?;

    Ok(Json(rows))
}

pub async fn list_endpoints(State(state): State<AppState>) -> ApiResult<Json<Vec<Endpoint>>> {
    let rows = sqlx::query_as::<_, Endpoint>(
        "SELECT id, slug, label, label_zh, host, port, notes, enabled FROM endpoints ORDER BY slug",
    )
    .fetch_all(state.db())
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEndpoint {
    pub slug: String,
    pub label: String,
    pub label_zh: String,
    pub host: String,
    pub port: i32,
    pub notes: Option<String>,
}

pub async fn create_endpoint(
    State(state): State<AppState>,
    Json(body): Json<NewEndpoint>,
) -> ApiResult<Json<Endpoint>> {
    if body.slug.trim().is_empty() {
        return Err(ApiError::BadRequest("slug is required".into()));
    }
    if !(1..=65535).contains(&body.port) {
        return Err(ApiError::BadRequest("port must be 1-65535".into()));
    }

    let row = sqlx::query_as::<_, Endpoint>(
        r#"
        INSERT INTO endpoints (slug, label, label_zh, host, port, notes)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (slug) DO UPDATE
            SET label = EXCLUDED.label,
                label_zh = EXCLUDED.label_zh,
                host = EXCLUDED.host,
                port = EXCLUDED.port,
                notes = EXCLUDED.notes
        RETURNING id, slug, label, label_zh, host, port, notes, enabled
        "#,
    )
    .bind(&body.slug)
    .bind(&body.label)
    .bind(&body.label_zh)
    .bind(&body.host)
    .bind(body.port)
    .bind(&body.notes)
    .fetch_one(state.db())
    .await?;

    Ok(Json(row))
}
