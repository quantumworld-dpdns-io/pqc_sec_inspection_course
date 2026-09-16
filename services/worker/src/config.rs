use std::collections::BTreeMap;
use std::time::Duration;

use clap::Parser;

#[derive(Clone, Debug, Parser)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "REDIS_URL", default_value = "redis://redis:6379")]
    pub redis_url: String,

    /// `name=base_url` pairs, comma separated. Adapters not listed here are reported as
    /// `disabled` (未啟用) rather than failed, which is exactly the distinction the KEM
    /// compatibility matrix draws.
    #[arg(
        long,
        env = "ADAPTERS",
        default_value = "openssl=http://adapter-openssl:9100,boringssl=http://adapter-boringssl:9100,wolfssl=http://adapter-wolfssl:9100,go=http://adapter-go:9100"
    )]
    pub adapters: String,

    /// How many probes run at once. Handshakes are cheap but lab targets are not, so this
    /// doubles as politeness towards the machine under test.
    #[arg(long, env = "WORKER_CONCURRENCY", default_value_t = 8)]
    pub concurrency: usize,

    #[arg(long, env = "WORKER_NAME", default_value = "worker-1")]
    pub name: String,

    #[arg(long, env = "PROBE_TIMEOUT_MS", default_value_t = 15_000)]
    pub probe_timeout_ms: u64,

    /// Reclaim jobs whose worker died mid-probe after this many milliseconds.
    #[arg(long, env = "CLAIM_STALE_MS", default_value_t = 120_000)]
    pub claim_stale_ms: u64,
}

impl Config {
    pub fn adapter_map(&self) -> BTreeMap<String, String> {
        self.adapters
            .split(',')
            .filter_map(|entry| {
                let (name, url) = entry.split_once('=')?;
                let name = name.trim();
                let url = url.trim().trim_end_matches('/');
                (!name.is_empty() && !url.is_empty()).then(|| (name.to_string(), url.to_string()))
            })
            .collect()
    }

    pub fn probe_timeout(&self) -> Duration {
        Duration::from_millis(self.probe_timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(adapters: &str) -> Config {
        Config {
            database_url: "postgres://x".into(),
            redis_url: "redis://x".into(),
            adapters: adapters.into(),
            concurrency: 1,
            name: "t".into(),
            probe_timeout_ms: 1,
            claim_stale_ms: 1,
        }
    }

    #[test]
    fn adapter_list_parses_and_trims() {
        let map = config("openssl=http://a:9100/, go = http://b:9100 ,broken").adapter_map();
        assert_eq!(map.get("openssl").unwrap(), "http://a:9100");
        assert_eq!(map.get("go").unwrap(), "http://b:9100");
        assert_eq!(map.len(), 2, "entries without = are skipped");
    }
}
