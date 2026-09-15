use std::sync::Arc;

use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub config: Config,
}

impl AppState {
    pub async fn connect(config: Config) -> anyhow::Result<Self> {
        let db = PgPoolOptions::new()
            .max_connections(config.db_max_connections)
            .connect(&config.database_url)
            .await?;
        let client = redis::Client::open(config.redis_url.clone())?;
        let redis = ConnectionManager::new(client).await?;
        Ok(Self(Arc::new(Inner { db, redis, config })))
    }

    pub fn db(&self) -> &PgPool {
        &self.0.db
    }

    pub fn redis(&self) -> ConnectionManager {
        self.0.redis.clone()
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    /// A second Redis connection, needed because pub/sub takes over a connection and the
    /// pooled `ConnectionManager` is busy serving commands.
    pub async fn pubsub_client(&self) -> anyhow::Result<redis::Client> {
        Ok(redis::Client::open(self.0.config.redis_url.clone())?)
    }
}
