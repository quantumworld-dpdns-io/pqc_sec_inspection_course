//! Client side of the adapter contract, plus a capability cache.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use adapter_proto::{Capabilities, ProbeRequest, ProbeResponse, Stage};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AdapterRegistry {
    http: reqwest::Client,
    bases: Arc<BTreeMap<String, String>>,
    capabilities: Arc<RwLock<BTreeMap<String, Capabilities>>>,
}

impl AdapterRegistry {
    pub fn new(bases: BTreeMap<String, String>, timeout: Duration) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            // The adapter itself enforces the per-handshake timeout; this is the outer
            // bound that covers a wedged container.
            .timeout(timeout + Duration::from_secs(5))
            .build()?;
        Ok(Self {
            http,
            bases: Arc::new(bases),
            capabilities: Arc::new(RwLock::new(BTreeMap::new())),
        })
    }

    pub fn names(&self) -> Vec<String> {
        self.bases.keys().cloned().collect()
    }

    pub fn is_configured(&self, adapter: &str) -> bool {
        self.bases.contains_key(adapter)
    }

    /// Capabilities change only when an image is rebuilt, so one successful fetch per
    /// process lifetime is enough.
    pub async fn capabilities(&self, adapter: &str) -> anyhow::Result<Capabilities> {
        if let Some(cached) = self.capabilities.read().await.get(adapter) {
            return Ok(cached.clone());
        }
        let base = self
            .bases
            .get(adapter)
            .ok_or_else(|| anyhow::anyhow!("adapter {adapter} is not configured"))?;
        let caps: Capabilities = self
            .http
            .get(format!("{base}/capabilities"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        self.capabilities
            .write()
            .await
            .insert(adapter.to_string(), caps.clone());
        Ok(caps)
    }

    /// Run one handshake. Transport-level problems come back as a `ProbeResponse` rather
    /// than an `Err`, so a dead adapter shows up as a red cell with a readable reason
    /// instead of stalling the whole task.
    pub async fn probe(&self, adapter: &str, request: &ProbeRequest) -> ProbeResponse {
        let Some(base) = self.bases.get(adapter) else {
            return ProbeResponse::failure(
                adapter,
                Stage::Tcp,
                format!("adapter {adapter} is not configured"),
            );
        };

        match self
            .http
            .post(format!("{base}/probe"))
            .json(request)
            .send()
            .await
        {
            Err(err) => {
                ProbeResponse::failure(adapter, Stage::Tcp, format!("adapter unreachable: {err}"))
            }
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return ProbeResponse::failure(
                        adapter,
                        Stage::Tcp,
                        format!("adapter returned {status}: {}", body.trim()),
                    );
                }
                match response.json::<ProbeResponse>().await {
                    Ok(parsed) => parsed,
                    Err(err) => ProbeResponse::failure(
                        adapter,
                        Stage::Tcp,
                        format!("adapter sent an unreadable report: {err}"),
                    ),
                }
            }
        }
    }
}
