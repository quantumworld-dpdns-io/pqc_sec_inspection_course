//! PQC KEM DEMO runs. Each mode writes the same streaming console the dev system had, so
//! the log lines below are part of the product, not debug output.

use adapter_proto::{ProbeRequest, ProbeResponse, Target};
use pqcas_domain::jobs::{kem_channel, KemEvent, KemJob};
use pqcas_domain::status::{KemRunMode, ResultStatus, TaskStatus};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::adapters::AdapterRegistry;

/// The adapter columns of the compatibility matrix. Columns without a configured adapter
/// render as 未啟用 (disabled) rather than as failures.
pub const MATRIX_ADAPTERS: [&str; 8] = [
    "openssl",
    "boringssl",
    "wolfssl",
    "go",
    "bouncycastle",
    "aws-lc",
    "nss",
    "circl",
];

/// The KEM rows of the compatibility matrix.
pub const MATRIX_KEMS: [&str; 6] = [
    "MLKEM1024",
    "MLKEM768",
    "SecP256r1MLKEM768",
    "SecP384r1MLKEM1024",
    "X25519Kyber768Draft00",
    "X25519MLKEM768",
];

/// Groups that provide no post-quantum protection. Ordering *within* this set is not a PQC
/// finding; a PQ group listed after any of them is.
///
/// The spellings are deliberately redundant: a recovered order carries whatever name the
/// stack reported, and OpenSSL alone calls P-256 `prime256v1` in its temp-key line. A
/// classical curve that fails to match here is misread as post-quantum and produces a bogus
/// "higher security placed later" finding.
const CLASSICAL: [&str; 11] = [
    "x25519",
    "x448",
    "secp256r1",
    "secp384r1",
    "secp521r1",
    "prime256v1",
    "prime384v1",
    "prime521v1",
    "p-256",
    "p-384",
    "p-521",
];

/// Coarse security level used for the descending-order check among PQ groups.
fn pq_level(group: &str) -> Option<u8> {
    let g = group.to_ascii_lowercase();
    if is_classical(&g) {
        return None;
    }
    Some(match () {
        _ if g.contains("1024") || g.contains("mceliece460896") => 5,
        _ if g.contains("768") || g.contains("976") => 3,
        _ if g.contains("512") || g.contains("640") => 1,
        _ => 3,
    })
}

fn is_classical(group: &str) -> bool {
    let g = group.to_ascii_lowercase();
    CLASSICAL.contains(&g.as_str())
}

pub struct Console<'a> {
    db: &'a PgPool,
    redis: &'a mut ConnectionManager,
    run_id: Uuid,
    seq: i32,
}

impl<'a> Console<'a> {
    pub fn new(db: &'a PgPool, redis: &'a mut ConnectionManager, run_id: Uuid) -> Self {
        Self {
            db,
            redis,
            run_id,
            seq: 0,
        }
    }

    pub async fn log(&mut self, level: &str, message: impl Into<String>) -> anyhow::Result<()> {
        let message = message.into();
        let at = chrono::Utc::now();
        self.seq += 1;

        sqlx::query(
            "INSERT INTO kem_run_logs (run_id, seq, at, level, message) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(self.run_id)
        .bind(self.seq)
        .bind(at)
        .bind(level)
        .bind(&message)
        .execute(self.db)
        .await?;

        let event = KemEvent::Log {
            seq: self.seq,
            at: at.to_rfc3339(),
            level: level.to_string(),
            message,
        };
        let _: i64 = self
            .redis
            .publish(kem_channel(self.run_id), serde_json::to_string(&event)?)
            .await?;
        Ok(())
    }

    pub async fn cell(
        &mut self,
        kem: &str,
        adapter: &str,
        verdict: ResultStatus,
        detail: Option<String>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO kem_matrix_cells (run_id, kem, adapter, verdict, detail)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (run_id, kem, adapter) DO UPDATE
                SET verdict = EXCLUDED.verdict, detail = EXCLUDED.detail
            "#,
        )
        .bind(self.run_id)
        .bind(kem)
        .bind(adapter)
        .bind(verdict)
        .bind(&detail)
        .execute(self.db)
        .await?;

        let event = KemEvent::Cell {
            kem: kem.to_string(),
            adapter: adapter.to_string(),
            verdict,
            detail,
        };
        let _: i64 = self
            .redis
            .publish(kem_channel(self.run_id), serde_json::to_string(&event)?)
            .await?;
        Ok(())
    }

    pub async fn finish(&mut self, status: TaskStatus) -> anyhow::Result<()> {
        sqlx::query("UPDATE kem_runs SET status = $2, finished_at = now() WHERE id = $1")
            .bind(self.run_id)
            .bind(status)
            .execute(self.db)
            .await?;
        let event = KemEvent::Finished {
            run_id: self.run_id,
        };
        let _: i64 = self
            .redis
            .publish(kem_channel(self.run_id), serde_json::to_string(&event)?)
            .await?;
        Ok(())
    }
}

pub async fn handle(
    db: &PgPool,
    redis: &mut ConnectionManager,
    adapters: &AdapterRegistry,
    job: KemJob,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE kem_runs SET status = 'running' WHERE id = $1")
        .bind(job.run_id)
        .execute(db)
        .await?;

    let mut console = Console::new(db, redis, job.run_id);
    let outcome = match job.mode {
        KemRunMode::LibraryCompat => library_compat(&mut console, adapters, &job).await,
        KemRunMode::GroupCompat => group_compat(&mut console, adapters, &job).await,
        KemRunMode::Priority => priority(&mut console, adapters, &job).await,
    };

    match outcome {
        Ok(()) => console.finish(TaskStatus::Finished).await?,
        Err(err) => {
            console
                .log("result", format!("Result: RunFailed - {err}"))
                .await?;
            console.finish(TaskStatus::Failed).await?;
        }
    }
    Ok(())
}

fn request_for(job: &KemJob, groups: Vec<String>) -> ProbeRequest {
    let mut request = ProbeRequest::new(Target {
        host: job.host.clone(),
        port: job.port,
        sni: Some(job.host.clone()),
        http_path: Some("/".to_string()),
    });
    request.kem_groups = groups;
    request
}

/// Tab 1 — 函式庫相容性測試: every KEM against every adapter.
async fn library_compat(
    console: &mut Console<'_>,
    adapters: &AdapterRegistry,
    job: &KemJob,
) -> anyhow::Result<()> {
    console
        .log(
            "info",
            format!("=== run_library_compat ({}:{}) ===", job.host, job.port),
        )
        .await?;

    for kem in MATRIX_KEMS {
        for adapter in MATRIX_ADAPTERS {
            if !adapters.is_configured(adapter) {
                console
                    .cell(kem, adapter, ResultStatus::Disabled, None)
                    .await?;
                continue;
            }

            let supported = match adapters.capabilities(adapter).await {
                Ok(caps) => caps.kem_groups.iter().any(|g| g.eq_ignore_ascii_case(kem)),
                Err(err) => {
                    console
                        .log(
                            "info",
                            format!("{adapter}: capabilities unavailable ({err})"),
                        )
                        .await?;
                    false
                }
            };
            if !supported {
                console
                    .cell(kem, adapter, ResultStatus::Unsupported, None)
                    .await?;
                continue;
            }

            let response = adapters
                .probe(adapter, &request_for(job, vec![kem.to_string()]))
                .await;
            let verdict = if response.handshake_success {
                ResultStatus::Passed
            } else {
                ResultStatus::Failed
            };
            console
                .cell(kem, adapter, verdict, response.error_summary.clone())
                .await?;
            console
                .log(
                    "info",
                    format!(
                        "{adapter} × {kem}: {}",
                        if response.handshake_success {
                            "PASS"
                        } else {
                            "FAIL"
                        }
                    ),
                )
                .await?;
        }
    }

    console
        .log("result", "Result: compatibility matrix complete")
        .await?;
    Ok(())
}

/// Tab 2 — 演算法群組相容性測試: one handshake offering the selected key_share groups.
async fn group_compat(
    console: &mut Console<'_>,
    adapters: &AdapterRegistry,
    job: &KemJob,
) -> anyhow::Result<()> {
    let adapter = preferred_adapter(adapters)?;
    console
        .log(
            "info",
            format!("=== run_full_handshake ({}:{}) ===", job.host, job.port),
        )
        .await?;
    console
        .log("info", format!("Using KEMs: {}", job.kem_groups.join(", ")))
        .await?;

    let response = adapters
        .probe(&adapter, &request_for(job, job.kem_groups.clone()))
        .await;

    if !response.handshake_success {
        console.log("info", "key_gen TLS 完整交握失敗：").await?;
        // Prefer the alert name: "handshake_failure" tells a student what happened, where
        // the raw OpenSSL error string tells them which C file it happened in.
        let reason = response
            .alert
            .as_ref()
            .map(|alert| alert.description.clone())
            .or_else(|| response.error_summary.clone())
            .unwrap_or_else(|| "unknown".into());
        console
            .log("result", format!("Result: HandshakeFailed - {reason}"))
            .await?;
        return Ok(());
    }

    if let Some(http) = &response.http {
        console
            .log("info", "========== HTTPS response header ==========")
            .await?;
        console
            .log(
                "info",
                format!("HTTP/1.1 {} {}", http.status, status_text(http.status)),
            )
            .await?;
        for header in &http.headers {
            console
                .log(
                    "info",
                    format!("{}: {}", header.label, header.values.join(", ")),
                )
                .await?;
        }
        console
            .log("info", "========== HTTPS response header ==========")
            .await?;
        console.log("info", "response success").await?;
    }

    console.log("result", result_line(&response)).await?;
    Ok(())
}

fn result_line(response: &ProbeResponse) -> String {
    format!(
        "Result: ApplicationData - {} {}; group={}; sent={} bytes; received={} bytes",
        response
            .negotiated
            .tls_version
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        response
            .negotiated
            .cipher_suite
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        response
            .negotiated
            .group
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        response.evidence.bytes_sent,
        response.evidence.bytes_received,
    )
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "",
    }
}

/// Tab 3 — 演算法優先級測試: recover the server's preference order by repeatedly offering
/// everything it has not picked yet, then judge the order against PQC best practice.
async fn priority(
    console: &mut Console<'_>,
    adapters: &AdapterRegistry,
    job: &KemJob,
) -> anyhow::Result<()> {
    let adapter = preferred_adapter(adapters)?;
    console
        .log(
            "info",
            format!("=== run_priority_test ({}:{}) ===", job.host, job.port),
        )
        .await?;
    console.log("info", "=== 演算法指定順序測試 ===").await?;

    let caps = adapters.capabilities(&adapter).await?;
    let mut remaining: Vec<String> = if job.kem_groups.is_empty() {
        caps.kem_groups.clone()
    } else {
        job.kem_groups.clone()
    };

    let mut order: Vec<String> = Vec::new();
    // Each round removes exactly the group the server chose, so the loop is bounded by the
    // offer list even if a server answers with something it was not offered.
    while !remaining.is_empty() {
        let response = adapters
            .probe(&adapter, &request_for(job, remaining.clone()))
            .await;
        if !response.handshake_success {
            break;
        }
        let Some(group) = response.negotiated.group.clone() else {
            break;
        };
        console.log("info", group.clone()).await?;
        order.push(group.clone());
        let before = remaining.len();
        remaining.retain(|g| !g.eq_ignore_ascii_case(&group));
        if remaining.len() == before {
            break;
        }
    }

    let findings = evaluate_order(&order);
    if findings.is_empty() {
        console
            .log(
                "finding_ok",
                "[+] 伺服器的演算法指定順序符合安全性等級從高到低的最佳實踐",
            )
            .await?;
    } else {
        for group in &findings {
            console
                .log(
                    "finding_warn",
                    format!("[-] 伺服器將安全性較高演算法放在後面：{group}"),
                )
                .await?;
        }
    }

    console
        .log(
            "result",
            format!(
                "Final Order: [{}]",
                order
                    .iter()
                    .map(|g| format!("\"{g}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
        .await?;
    Ok(())
}

/// Groups that sit behind something weaker than themselves. A post-quantum group placed
/// after any classical group is the headline finding; PQ groups out of level order are the
/// same kind of mistake at a smaller scale.
pub fn evaluate_order(order: &[String]) -> Vec<String> {
    let mut findings = Vec::new();
    let mut seen_classical = false;
    let mut best_pq_so_far: Option<u8> = None;

    for group in order {
        match pq_level(group) {
            None => seen_classical = true,
            Some(level) => {
                // Two ways to be misplaced: behind a classical group, or behind a weaker
                // post-quantum one. Both mean the server prefers less security first.
                let behind_classical = seen_classical;
                let behind_weaker_pq = best_pq_so_far.is_some_and(|best| level > best);
                if behind_classical || behind_weaker_pq {
                    findings.push(group.clone());
                }
                best_pq_so_far = Some(best_pq_so_far.map_or(level, |best| best.min(level)));
            }
        }
    }
    findings
}

/// The KEM DEMO drives a single stack; prefer OpenSSL because it carries the widest PQC
/// group list, and fall back to whatever the deployment does have.
fn preferred_adapter(adapters: &AdapterRegistry) -> anyhow::Result<String> {
    for candidate in ["openssl", "go", "boringssl", "wolfssl"] {
        if adapters.is_configured(candidate) {
            return Ok(candidate.to_string());
        }
    }
    adapters
        .names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no adapters are configured"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pq_first_descending_order_is_clean() {
        // The B-server order from the dev system, which it reported as best practice.
        let order = v(&[
            "MLKEM1024",
            "MLKEM768",
            "x25519MLKEM768",
            "x25519",
            "secp256r1",
            "secp384r1",
        ]);
        assert!(evaluate_order(&order).is_empty());
    }

    #[test]
    fn pq_after_classical_is_flagged() {
        // The A-server order, which the dev system flagged group by group.
        let order = v(&[
            "x25519",
            "secp256r1",
            "secp384r1",
            "MLKEM512",
            "MLKEM768",
            "MLKEM1024",
        ]);
        let findings = evaluate_order(&order);
        assert_eq!(findings, v(&["MLKEM512", "MLKEM768", "MLKEM1024"]));
    }

    #[test]
    fn stronger_pq_behind_weaker_pq_is_flagged() {
        let findings = evaluate_order(&v(&["MLKEM768", "MLKEM1024"]));
        assert_eq!(findings, v(&["MLKEM1024"]));
    }

    #[test]
    fn openssl_curve_spellings_are_recognised_as_classical() {
        // Exactly what a priority run against the lab's B server recovers.
        let order = v(&[
            "MLKEM1024",
            "MLKEM768",
            "X25519MLKEM768",
            "X25519",
            "prime256v1",
        ]);
        assert!(
            evaluate_order(&order).is_empty(),
            "{:?}",
            evaluate_order(&order)
        );
    }

    #[test]
    fn classical_groups_are_not_ranked_against_each_other() {
        assert!(evaluate_order(&v(&["secp256r1", "secp384r1", "x25519"])).is_empty());
    }
}
