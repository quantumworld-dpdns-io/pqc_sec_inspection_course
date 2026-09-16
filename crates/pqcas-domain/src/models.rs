//! Records as they cross the API boundary. Field names are camelCase on the wire to keep
//! the Next.js side idiomatic; the SQL columns stay snake_case.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::status::{Capability, ExecStatus, KemRunMode, ReportStatus, ResultStatus, TaskStatus};

/// A lab target ("A server", "正常 server", "漏洞 server", ...) shown on the Environment
/// page and in the KEM DEMO endpoint dropdown.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: Uuid,
    pub slug: String,
    pub label: String,
    pub label_zh: String,
    pub host: String,
    pub port: i32,
    pub notes: Option<String>,
    pub enabled: bool,
}

/// One entry of the seeded algorithm catalog. `enabled = false` renders greyed out, which
/// is how the KEM DEMO screen shows algorithms the lab has not turned on.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Algorithm {
    pub id: i32,
    /// `kem_group` or `sig_alg`.
    pub kind: String,
    pub name: String,
    pub display_name: String,
    pub family: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Test {
    pub id: Uuid,
    pub host: String,
    pub port: i32,
    pub http_path: String,
    pub method: String,
    pub tool: String,
    pub plugin: String,
    pub status: TaskStatus,
    pub group_count: i32,
    pub sig_alg_count: i32,
    pub subtask_count: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Test {
    /// The `host:port` string used as the page title and breadcrumb leaf.
    pub fn display_target(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub id: Uuid,
    pub test_id: Uuid,
    pub ordinal: i32,
    pub group_name: String,
    pub sig_alg: String,
    pub exec_status: ExecStatus,
    pub result_status: ResultStatus,
    pub report_status: ReportStatus,
    pub failed_stage: Option<String>,
    pub error_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// The heavy half of a subtask, split into its own table so matrix queries stay cheap.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskReport {
    pub subtask_id: Uuid,
    pub adapter: String,
    pub tls_summary: serde_json::Value,
    pub messages: serde_json::Value,
    pub cert_chain: serde_json::Value,
    pub http: Option<serde_json::Value>,
    pub diagnostics: serde_json::Value,
    pub raw: serde_json::Value,
    pub evidence: Option<String>,
    pub duration_ms: i64,
}

/// Matrix cell as served to the UI: everything needed to paint and link a chip.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCell {
    pub subtask_id: Uuid,
    pub group_name: String,
    pub sig_alg: String,
    pub result_status: ResultStatus,
    pub exec_status: ExecStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Matrix {
    pub groups: Vec<String>,
    pub sig_algs: Vec<String>,
    pub cells: Vec<MatrixCell>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct KemRun {
    pub id: Uuid,
    pub mode: KemRunMode,
    pub endpoint_id: Uuid,
    pub status: TaskStatus,
    pub kem_groups: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// One line of the `執行紀錄 (OUTPUT)` console.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct KemRunLog {
    pub run_id: Uuid,
    pub seq: i32,
    pub at: DateTime<Utc>,
    /// `info` | `finding_ok` | `finding_warn` | `result`
    pub level: String,
    pub message: String,
}

/// One cell of the KEM \ ADAPTER compatibility matrix.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct KemMatrixCell {
    pub run_id: Uuid,
    pub kem: String,
    pub adapter: String,
    pub verdict: ResultStatus,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CavpSession {
    pub id: Uuid,
    pub capability: Capability,
    pub parameter_set: String,
    pub vs_id: i64,
    pub prompt: serde_json::Value,
    pub submitted: Option<serde_json::Value>,
    pub validation: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
