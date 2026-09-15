//! CAVP 預檢測體驗站: hand out fixed question packs, grade submitted results.
//!
//! The answer key never leaves the server — `GET /cavp/packs/{slug}` returns the prompt and
//! the capability registration only.

use axum::extract::{Path, State};
use axum::Json;
use cavp_validate::{AnswerKey, ValidationReport};
use pqcas_domain::status::Capability;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    pub slug: String,
    pub capability: Capability,
    pub parameter_set: String,
    pub vs_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackDetail {
    pub slug: String,
    pub capability: Capability,
    pub parameter_set: String,
    pub vs_id: i64,
    /// Rendered by step 01 (能力宣告).
    pub registration: serde_json::Value,
    /// Rendered by step 02 (查看題包) and copied by the student.
    pub prompt: serde_json::Value,
    /// tcId 1 ships pre-filled as a worked example, exactly as the dev system did.
    pub example: serde_json::Value,
}

pub async fn list_packs(State(state): State<AppState>) -> ApiResult<Json<Vec<PackSummary>>> {
    let rows = sqlx::query_as::<_, PackSummary>(
        "SELECT slug, capability, parameter_set, vs_id FROM cavp_packs ORDER BY vs_id",
    )
    .fetch_all(state.db())
    .await?;
    Ok(Json(rows))
}

pub async fn get_pack(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<PackDetail>> {
    let (capability, parameter_set, vs_id, prompt, registration, answer_key) =
        fetch_pack(&state, &slug).await?;

    Ok(Json(PackDetail {
        slug,
        capability,
        parameter_set,
        vs_id,
        registration,
        prompt,
        example: example_answer(&answer_key),
    }))
}

/// Only tcId 1 of the key is exposed, which is what makes step 04's "tcId 1 已預填範例"
/// possible without giving away the rest of the pack.
fn example_answer(answer_key: &serde_json::Value) -> serde_json::Value {
    answer_key
        .get("cases")
        .and_then(|c| c.as_array())
        .and_then(|cases| cases.iter().find(|c| c.get("tcId").and_then(|v| v.as_i64()) == Some(1)))
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSession {
    pub slug: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub id: Uuid,
    pub slug: String,
    pub capability: Capability,
    pub parameter_set: String,
    pub vs_id: i64,
    pub prompt: serde_json::Value,
    pub submitted: Option<serde_json::Value>,
    pub validation: Option<serde_json::Value>,
}

pub async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<NewSession>,
) -> ApiResult<Json<SessionDetail>> {
    let (capability, parameter_set, vs_id, prompt, _registration, _key) =
        fetch_pack(&state, &body.slug).await?;

    let pack_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM cavp_packs WHERE slug = $1")
        .bind(&body.slug)
        .fetch_one(state.db())
        .await?;

    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO cavp_sessions (pack_id, capability, parameter_set, vs_id, prompt)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(pack_id)
    .bind(capability)
    .bind(&parameter_set)
    .bind(vs_id)
    .bind(&prompt)
    .fetch_one(state.db())
    .await?;

    Ok(Json(SessionDetail {
        id,
        slug: body.slug,
        capability,
        parameter_set,
        vs_id,
        prompt,
        submitted: None,
        validation: None,
    }))
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SessionDetail>> {
    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Capability,
            String,
            i64,
            serde_json::Value,
            Option<serde_json::Value>,
            Option<serde_json::Value>,
        ),
    >(
        r#"
        SELECT s.id, p.slug, s.capability, s.parameter_set, s.vs_id, s.prompt, s.submitted, s.validation
        FROM cavp_sessions s JOIN cavp_packs p ON p.id = s.pack_id
        WHERE s.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("cavp session"))?;

    Ok(Json(SessionDetail {
        id: row.0,
        slug: row.1,
        capability: row.2,
        parameter_set: row.3,
        vs_id: row.4,
        prompt: row.5,
        submitted: row.6,
        validation: row.7,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    /// The contents of the step 04 textarea, submitted as text so a malformed payload can
    /// be reported to the student instead of rejected by the JSON body parser.
    pub results: String,
}

pub async fn validate_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Submission>,
) -> ApiResult<Json<ValidationReport>> {
    let pack_id = sqlx::query_scalar::<_, Uuid>("SELECT pack_id FROM cavp_sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(state.db())
        .await?
        .ok_or(ApiError::NotFound("cavp session"))?;

    let answer_key_json =
        sqlx::query_scalar::<_, serde_json::Value>("SELECT answer_key FROM cavp_packs WHERE id = $1")
            .bind(pack_id)
            .fetch_one(state.db())
            .await?;

    let answer_key: AnswerKey = serde_json::from_value(answer_key_json)?;

    let report = cavp_validate::validate(&body.results, &answer_key)
        .map_err(|err| ApiError::BadRequest(err.to_string()))?;

    let submitted: serde_json::Value =
        serde_json::from_str(&body.results).unwrap_or(serde_json::Value::Null);

    sqlx::query("UPDATE cavp_sessions SET submitted = $2, validation = $3 WHERE id = $1")
        .bind(id)
        .bind(&submitted)
        .bind(serde_json::to_value(&report)?)
        .execute(state.db())
        .await?;

    Ok(Json(report))
}

type PackRow = (
    Capability,
    String,
    i64,
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
);

async fn fetch_pack(state: &AppState, slug: &str) -> ApiResult<PackRow> {
    sqlx::query_as::<_, PackRow>(
        r#"
        SELECT capability, parameter_set, vs_id, prompt, registration, answer_key
        FROM cavp_packs WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(state.db())
    .await?
    .ok_or(ApiError::NotFound("cavp pack"))
}
