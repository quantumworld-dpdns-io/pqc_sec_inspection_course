//! One adapter binary, three TLS stacks.
//!
//! OpenSSL, BoringSSL and wolfSSL all ship a command-line client that can be pinned to a
//! specific group and made to talk TLS 1.3, so the adapter drives that client and parses
//! its output rather than linking three C libraries into one process. Each image sets
//! `ADAPTER_BACKEND` and its own capability list; the `mock` backend keeps CI and the
//! Playwright suite honest without needing a PQC-capable server.

mod backend;
mod server;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "pqcas-adapter")]
pub struct Config {
    /// `openssl` | `boringssl` | `wolfssl` | `mock`
    #[arg(long, env = "ADAPTER_BACKEND", default_value = "mock")]
    pub backend: String,

    /// Reported as `Capabilities.adapter`; defaults to the backend name.
    #[arg(long, env = "ADAPTER_NAME")]
    pub name: Option<String>,

    #[arg(long, env = "ADAPTER_BIND", default_value = "0.0.0.0:9100")]
    pub bind: String,

    /// Path to the stack's client binary.
    #[arg(long, env = "ADAPTER_CLIENT_BIN", default_value = "openssl")]
    pub client_bin: String,

    /// Comma-separated TLS groups this stack can offer.
    #[arg(long, env = "ADAPTER_KEM_GROUPS", default_value = "")]
    pub kem_groups: String,

    /// Comma-separated signature algorithms this stack can offer.
    #[arg(long, env = "ADAPTER_SIG_ALGS", default_value = "")]
    pub sig_algs: String,

    #[arg(long, env = "ADAPTER_VERSION", default_value = "unknown")]
    pub version: String,

    /// Directory to run the client binary from. wolfSSL's example client refuses to start
    /// unless its working directory contains the `certs/` tree it was built with.
    #[arg(long, env = "ADAPTER_WORKDIR")]
    pub workdir: Option<String>,
}

impl Config {
    pub fn adapter_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.backend.clone())
    }

    pub fn kem_group_list(&self) -> Vec<String> {
        split_list(&self.kem_groups)
    }

    pub fn sig_alg_list(&self) -> Vec<String> {
        split_list(&self.sig_algs)
    }
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::parse();
    server::serve(config).await
}

#[cfg(test)]
mod tests {
    use super::split_list;

    #[test]
    fn capability_lists_tolerate_spacing_and_blanks() {
        assert_eq!(split_list(" a, b ,,c "), vec!["a", "b", "c"]);
        assert!(split_list("").is_empty());
    }
}
