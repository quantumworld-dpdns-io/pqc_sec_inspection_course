//! Handling of one PQ-CAS subtask: probe, classify, persist, publish.

use adapter_proto::{CaptureOptions, ProbeRequest, Target};
use pqcas_domain::jobs::{test_channel, ProbeJob, TestEvent};
use pqcas_domain::status::{ExecStatus, ResultStatus};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;

use crate::adapters::AdapterRegistry;
use crate::store;

pub async fn handle(
    db: &PgPool,
    redis: &mut ConnectionManager,
    adapters: &AdapterRegistry,
    job: ProbeJob,
) -> anyhow::Result<()> {
    store::mark_test_running(db, job.test_id).await?;
    store::mark_subtask_running(db, job.subtask_id).await?;

    if !adapters.is_configured(&job.adapter) {
        // The lab has not built this stack: the UI greys the cell out rather than blaming
        // the target.
        store::mark_subtask_error(
            db,
            job.subtask_id,
            &format!("adapter {} is not enabled in this deployment", job.adapter),
        )
        .await?;
        publish(redis, &job, ExecStatus::Error, ResultStatus::Disabled).await?;
        finish(db, redis, &job).await?;
        return Ok(());
    }

    let mut request = ProbeRequest::new(Target {
        host: job.host.clone(),
        port: job.port,
        sni: Some(job.host.clone()),
        http_path: Some(job.http_path.clone()),
    });
    request.kem_groups = vec![job.group_name.clone()];
    request.sig_algs = vec![job.sig_alg.clone()];
    request.timeout_ms = job.timeout_ms;
    // Raw capture on every subtask would balloon the database; failures are where the
    // evidence panel earns its keep, so ask for it and drop it when the handshake works.
    request.capture = CaptureOptions {
        raw_records: true,
        keylog: false,
    };

    let response = adapters.probe(&job.adapter, &request).await;

    let offered = vec![job.group_name.clone(), job.sig_alg.clone()];
    let result = match adapters.capabilities(&job.adapter).await {
        Ok(caps) => pqcas_domain::classify(&response, &offered, &caps),
        Err(err) => {
            tracing::warn!(adapter = %job.adapter, %err, "capabilities unavailable, falling back to raw outcome");
            if response.handshake_success {
                ResultStatus::Passed
            } else {
                ResultStatus::Failed
            }
        }
    };

    store::save_report(db, job.subtask_id, result, &response).await?;
    publish(redis, &job, ExecStatus::Finished, result).await?;
    finish(db, redis, &job).await?;
    Ok(())
}

async fn publish(
    redis: &mut ConnectionManager,
    job: &ProbeJob,
    exec_status: ExecStatus,
    result_status: ResultStatus,
) -> anyhow::Result<()> {
    let event = TestEvent::SubtaskUpdated {
        subtask_id: job.subtask_id,
        exec_status,
        result_status,
        group_name: job.group_name.clone(),
        sig_alg: job.sig_alg.clone(),
    };
    let payload = serde_json::to_string(&event)?;
    let _: i64 = redis.publish(test_channel(job.test_id), payload).await?;
    Ok(())
}

async fn finish(db: &PgPool, redis: &mut ConnectionManager, job: &ProbeJob) -> anyhow::Result<()> {
    if let Some(progress) = store::finish_test_if_done(db, job.test_id).await? {
        let event = TestEvent::TestFinished {
            test_id: job.test_id,
            passed: progress.passed as u32,
            failed: progress.failed as u32,
        };
        let payload = serde_json::to_string(&event)?;
        let _: i64 = redis.publish(test_channel(job.test_id), payload).await?;
    }
    Ok(())
}
