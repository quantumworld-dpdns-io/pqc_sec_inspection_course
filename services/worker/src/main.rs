mod adapters;
mod config;
mod kem;
mod probe;
mod store;
mod stream;

use std::sync::Arc;

use clap::Parser;
use pqcas_domain::jobs::{KemJob, ProbeJob, STREAM_KEM, STREAM_PROBE};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Semaphore;
use tracing_subscriber::EnvFilter;

use crate::adapters::AdapterRegistry;
use crate::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let config = Config::parse();
    let db = PgPoolOptions::new()
        .max_connections(config.concurrency as u32 + 2)
        .connect(&config.database_url)
        .await?;

    let client = redis::Client::open(config.redis_url.clone())?;
    let mut conn = redis::aio::ConnectionManager::new(client).await?;

    stream::ensure_group(&mut conn, STREAM_PROBE).await?;
    stream::ensure_group(&mut conn, STREAM_KEM).await?;

    let registry = AdapterRegistry::new(config.adapter_map(), config.probe_timeout())?;
    tracing::info!(adapters = ?registry.names(), worker = %config.name, "worker ready");

    let permits = Arc::new(Semaphore::new(config.concurrency));
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received");
                break;
            }
            result = tick(&db, &mut conn, &registry, &config, permits.clone()) => {
                if let Err(err) = result {
                    tracing::error!(%err, "worker tick failed; backing off");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    // Let in-flight probes finish before the process exits, so their reports land.
    let _ = permits.acquire_many(config.concurrency as u32).await;
    Ok(())
}

async fn tick(
    db: &sqlx::PgPool,
    conn: &mut redis::aio::ConnectionManager,
    registry: &AdapterRegistry,
    config: &Config,
    permits: Arc<Semaphore>,
) -> anyhow::Result<()> {
    let mut deliveries = stream::read(conn, &[STREAM_PROBE, STREAM_KEM], &config.name, 2_000, 16).await?;

    // Nothing new: use the idle moment to pick up anything a dead worker left pending.
    if deliveries.is_empty() {
        for stream_name in [STREAM_PROBE, STREAM_KEM] {
            deliveries.extend(
                stream::autoclaim(conn, stream_name, &config.name, config.claim_stale_ms).await?,
            );
        }
    }

    for delivery in deliveries {
        let permit = permits.clone().acquire_owned().await?;
        let db = db.clone();
        let registry = registry.clone();
        let mut worker_conn = conn.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let outcome = match delivery.stream.as_str() {
                STREAM_PROBE => match serde_json::from_str::<ProbeJob>(&delivery.payload) {
                    Ok(job) => probe::handle(&db, &mut worker_conn, &registry, job).await,
                    Err(err) => Err(anyhow::anyhow!("undecodable probe job: {err}")),
                },
                STREAM_KEM => match serde_json::from_str::<KemJob>(&delivery.payload) {
                    Ok(job) => kem::handle(&db, &mut worker_conn, &registry, job).await,
                    Err(err) => Err(anyhow::anyhow!("undecodable kem job: {err}")),
                },
                other => Err(anyhow::anyhow!("unknown stream {other}")),
            };

            if let Err(err) = outcome {
                tracing::error!(stream = %delivery.stream, id = %delivery.id, %err, "job failed");
            }

            // Acknowledge either way: a job that fails deterministically would otherwise be
            // reclaimed forever. Failures are already recorded on the subtask/run row.
            if let Err(err) = stream::ack(&mut worker_conn, &delivery.stream, &delivery.id).await {
                tracing::error!(%err, "failed to ack job");
            }
        });
    }

    Ok(())
}
