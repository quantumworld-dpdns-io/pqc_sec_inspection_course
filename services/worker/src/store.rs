//! All database writes the worker performs, kept in one place so the status transitions
//! are easy to audit.

use adapter_proto::ProbeResponse;
use pqcas_domain::status::{ExecStatus, ReportStatus, ResultStatus, TaskStatus};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn mark_subtask_running(db: &PgPool, subtask_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE subtasks SET exec_status = 'running', started_at = now() WHERE id = $1")
        .bind(subtask_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Mark the parent test as running the first time any of its subtasks starts.
pub async fn mark_test_running(db: &PgPool, test_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE tests SET status = 'running', started_at = coalesce(started_at, now()) WHERE id = $1 AND status = 'pending'",
    )
    .bind(test_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn save_report(
    db: &PgPool,
    subtask_id: Uuid,
    result: ResultStatus,
    response: &ProbeResponse,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;

    let tls_summary = json!({
        "handshakeSuccess": response.handshake_success,
        "tlsVersion": response.negotiated.tls_version,
        "cipherSuite": response.negotiated.cipher_suite,
        "keyExchangeGroup": response.negotiated.group,
        "certVerifySigAlg": response.negotiated.cert_verify_sig_alg,
        "alert": response.alert,
    });

    // A report is `complete` only when the probe actually finished the flow it was asked
    // to run; anything short of that is `partial`, which the drawer renders as such.
    let report_status = if response.handshake_success {
        ReportStatus::Complete
    } else if response.messages.is_empty() {
        ReportStatus::Partial
    } else {
        ReportStatus::Complete
    };

    sqlx::query(
        r#"
        INSERT INTO subtask_reports
            (subtask_id, adapter, tls_summary, messages, cert_chain, http, diagnostics, raw, evidence, duration_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (subtask_id) DO UPDATE SET
            adapter = EXCLUDED.adapter,
            tls_summary = EXCLUDED.tls_summary,
            messages = EXCLUDED.messages,
            cert_chain = EXCLUDED.cert_chain,
            http = EXCLUDED.http,
            diagnostics = EXCLUDED.diagnostics,
            raw = EXCLUDED.raw,
            evidence = EXCLUDED.evidence,
            duration_ms = EXCLUDED.duration_ms
        "#,
    )
    .bind(subtask_id)
    .bind(&response.adapter)
    .bind(&tls_summary)
    .bind(serde_json::to_value(&response.messages)?)
    .bind(serde_json::to_value(&response.cert_chain)?)
    .bind(serde_json::to_value(&response.http)?)
    .bind(&response.diagnostics)
    .bind(serde_json::to_value(response)?)
    .bind(response.evidence.uds_dump_b64.as_deref())
    .bind(response.duration_ms as i64)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE subtasks
        SET exec_status = $2, result_status = $3, report_status = $4,
            failed_stage = $5, error_summary = $6, finished_at = now()
        WHERE id = $1
        "#,
    )
    .bind(subtask_id)
    .bind(ExecStatus::Finished)
    .bind(result)
    .bind(report_status)
    .bind(response.failed_stage.as_str())
    .bind(response.error_summary.as_deref())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Record a worker-side failure (bad job payload, adapter registry miss) without a report.
pub async fn mark_subtask_error(db: &PgPool, subtask_id: Uuid, reason: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE subtasks
        SET exec_status = 'error', result_status = 'failed', report_status = 'missing',
            error_summary = $2, finished_at = now()
        WHERE id = $1
        "#,
    )
    .bind(subtask_id)
    .bind(reason)
    .execute(db)
    .await?;
    Ok(())
}

pub struct TestProgress {
    pub passed: i64,
    pub failed: i64,
}

/// Close the test out once nothing is left queued or running.
pub async fn finish_test_if_done(
    db: &PgPool,
    test_id: Uuid,
) -> anyhow::Result<Option<TestProgress>> {
    let (outstanding, passed, failed, errored, total) =
        sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            r#"
        SELECT
            count(*) FILTER (WHERE exec_status IN ('queued', 'running')),
            count(*) FILTER (WHERE result_status = 'passed'),
            count(*) FILTER (WHERE result_status IN ('failed', 'unsupported')),
            count(*) FILTER (WHERE exec_status = 'error'),
            count(*)
        FROM subtasks WHERE test_id = $1
        "#,
        )
        .bind(test_id)
        .fetch_one(db)
        .await?;

    if outstanding > 0 {
        return Ok(None);
    }

    // `failed` means the target failed a handshake — that is a finished test with red
    // cells, not a broken run. Only a task where every subtask errored out on our side
    // (adapter down, bad job) is a failed task.
    let status = if errored == total && total > 0 {
        TaskStatus::Failed
    } else {
        TaskStatus::Finished
    };

    sqlx::query("UPDATE tests SET status = $2, finished_at = now() WHERE id = $1")
        .bind(test_id)
        .bind(status)
        .execute(db)
        .await?;

    Ok(Some(TestProgress { passed, failed }))
}
