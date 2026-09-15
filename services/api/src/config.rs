use std::time::Duration;

use clap::Parser;

/// Everything the API needs, sourced from the environment so the same image runs in
/// compose and on the lab host without a config file.
#[derive(Clone, Debug, Parser)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "REDIS_URL", default_value = "redis://redis:6379")]
    pub redis_url: String,

    #[arg(long, env = "API_BIND", default_value = "0.0.0.0:8080")]
    pub bind: String,

    #[arg(long, env = "DB_MAX_CONNECTIONS", default_value_t = 16)]
    pub db_max_connections: u32,

    /// Upper bound on a single test task, guarding against a student pasting the entire
    /// catalog into both axes (101 algorithms squared is not a lab exercise).
    #[arg(long, env = "MAX_SUBTASKS_PER_TEST", default_value_t = 512)]
    pub max_subtasks_per_test: usize,

    #[arg(long, env = "PROBE_TIMEOUT_MS", default_value_t = 15_000)]
    pub probe_timeout_ms: u64,

    /// Which adapter a `tool/plugin` pair resolves to when the request does not say.
    #[arg(long, env = "DEFAULT_ADAPTER", default_value = "openssl")]
    pub default_adapter: String,

    #[arg(long, env = "CORS_ALLOW_ORIGIN", default_value = "*")]
    pub cors_allow_origin: String,

    #[arg(long, env = "SEED_DIR", default_value = "/app/seeds")]
    pub seed_dir: String,
}

impl Config {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
}
