//! Redis side of the API: enqueue work, publish nothing (the worker does that).

use pqcas_domain::jobs::{KemJob, ProbeJob, STREAM_KEM, STREAM_PROBE};
use redis::AsyncCommands;

use crate::error::ApiResult;
use crate::state::AppState;

/// Push probe jobs onto the stream. Called once per test creation with every subtask, so
/// it pipelines instead of round-tripping per job.
pub async fn enqueue_probes(state: &AppState, jobs: &[ProbeJob]) -> ApiResult<()> {
    let mut conn = state.redis();
    let mut pipe = redis::pipe();
    for job in jobs {
        let payload = serde_json::to_string(job)?;
        pipe.cmd("XADD")
            .arg(STREAM_PROBE)
            .arg("*")
            .arg("job")
            .arg(payload)
            .ignore();
    }
    pipe.query_async::<()>(&mut conn).await?;
    Ok(())
}

pub async fn enqueue_kem(state: &AppState, job: &KemJob) -> ApiResult<()> {
    let mut conn = state.redis();
    let payload = serde_json::to_string(job)?;
    let _: String = conn.xadd(STREAM_KEM, "*", &[("job", payload)]).await?;
    Ok(())
}
