//! Payloads placed on the Redis streams, and the events published back out for SSE.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::status::{ExecStatus, KemRunMode, ResultStatus};

pub const STREAM_PROBE: &str = "jobs:probe";
pub const STREAM_KEM: &str = "jobs:kem";
pub const CONSUMER_GROUP: &str = "workers";

pub fn test_channel(test_id: Uuid) -> String {
    format!("events:test:{test_id}")
}

pub fn kem_channel(run_id: Uuid) -> String {
    format!("events:kem:{run_id}")
}

/// One subtask handed to a worker: a single handshake against one target.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeJob {
    pub test_id: Uuid,
    pub subtask_id: Uuid,
    pub host: String,
    pub port: u16,
    pub http_path: String,
    pub adapter: String,
    pub group_name: String,
    pub sig_alg: String,
    pub timeout_ms: u64,
}

/// A KEM DEMO run; the worker expands it into many handshakes itself because the modes
/// need sequential decisions (priority probing narrows the offer list each round).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KemJob {
    pub run_id: Uuid,
    pub mode: KemRunMode,
    pub host: String,
    pub port: u16,
    pub kem_groups: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TestEvent {
    #[serde(rename_all = "camelCase")]
    SubtaskUpdated {
        subtask_id: Uuid,
        exec_status: ExecStatus,
        result_status: ResultStatus,
        group_name: String,
        sig_alg: String,
    },
    #[serde(rename_all = "camelCase")]
    TestFinished { test_id: Uuid, passed: u32, failed: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum KemEvent {
    #[serde(rename_all = "camelCase")]
    Log { seq: i32, at: String, level: String, message: String },
    #[serde(rename_all = "camelCase")]
    Cell { kem: String, adapter: String, verdict: ResultStatus, detail: Option<String> },
    #[serde(rename_all = "camelCase")]
    Finished { run_id: Uuid },
}
