//! Idempotent seeding of the catalog, lab endpoints and CAVP question packs.
//!
//! Run as a one-shot container before the API starts (`pqcas-api seed`). Every statement is
//! an upsert so re-running after a redeploy is a no-op rather than a duplicate-key error.

use std::path::Path;

use serde::Deserialize;
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlgorithmSeed {
    kind: String,
    name: String,
    display_name: String,
    family: Option<String>,
    enabled: bool,
    sort_order: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndpointSeed {
    slug: String,
    label: String,
    label_zh: String,
    host: String,
    port: i32,
    notes: Option<String>,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackSeed {
    slug: String,
    capability: String,
    parameter_set: String,
    vs_id: i64,
    prompt: serde_json::Value,
    answer_key: serde_json::Value,
    registration: serde_json::Value,
}

pub async fn run(db: &PgPool, dir: &str) -> anyhow::Result<()> {
    let dir = Path::new(dir);

    let algorithms: Vec<AlgorithmSeed> = read_json(&dir.join("algorithms.json"))?;
    for alg in &algorithms {
        sqlx::query(
            r#"
            INSERT INTO algorithms (kind, name, display_name, family, enabled, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (kind, name) DO UPDATE
                SET display_name = EXCLUDED.display_name,
                    family = EXCLUDED.family,
                    enabled = EXCLUDED.enabled,
                    sort_order = EXCLUDED.sort_order
            "#,
        )
        .bind(&alg.kind)
        .bind(&alg.name)
        .bind(&alg.display_name)
        .bind(&alg.family)
        .bind(alg.enabled)
        .bind(alg.sort_order)
        .execute(db)
        .await?;
    }

    let endpoints: Vec<EndpointSeed> = read_json(&dir.join("endpoints.json"))?;
    for endpoint in &endpoints {
        sqlx::query(
            r#"
            INSERT INTO endpoints (slug, label, label_zh, host, port, notes, enabled)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (slug) DO UPDATE
                SET label = EXCLUDED.label,
                    label_zh = EXCLUDED.label_zh,
                    host = EXCLUDED.host,
                    port = EXCLUDED.port,
                    notes = EXCLUDED.notes,
                    enabled = EXCLUDED.enabled
            "#,
        )
        .bind(&endpoint.slug)
        .bind(&endpoint.label)
        .bind(&endpoint.label_zh)
        .bind(&endpoint.host)
        .bind(endpoint.port)
        .bind(&endpoint.notes)
        .bind(endpoint.enabled)
        .execute(db)
        .await?;
    }

    let packs: Vec<PackSeed> = read_json(&dir.join("cavp_packs.json"))?;
    for pack in &packs {
        sqlx::query(
            r#"
            INSERT INTO cavp_packs (slug, capability, parameter_set, vs_id, prompt, answer_key, registration)
            VALUES ($1, $2::cavp_capability, $3, $4, $5, $6, $7)
            ON CONFLICT (slug) DO UPDATE
                SET capability = EXCLUDED.capability,
                    parameter_set = EXCLUDED.parameter_set,
                    vs_id = EXCLUDED.vs_id,
                    prompt = EXCLUDED.prompt,
                    answer_key = EXCLUDED.answer_key,
                    registration = EXCLUDED.registration
            "#,
        )
        .bind(&pack.slug)
        .bind(&pack.capability)
        .bind(&pack.parameter_set)
        .bind(pack.vs_id)
        .bind(&pack.prompt)
        .bind(&pack.answer_key)
        .bind(&pack.registration)
        .execute(db)
        .await?;
    }

    tracing::info!(
        algorithms = algorithms.len(),
        endpoints = endpoints.len(),
        packs = packs.len(),
        "seed complete"
    );
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<Vec<T>> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("cannot read seed file {}: {err}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}
