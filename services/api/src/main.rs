mod config;
mod error;
mod queue;
mod routes;
mod seed;
mod state;

use clap::{Parser, Subcommand};
use config::Config;
use state::AppState;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "pqcas-api", about = "PQ-CAS / KEM DEMO / CAVP API")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    config: Config,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the HTTP API (default).
    Serve,
    /// Apply pending SQL migrations and exit.
    Migrate,
    /// Upsert catalog, endpoints and CAVP packs from the seed directory, then exit.
    Seed,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let cli = Cli::parse();
    let config = cli.config;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Migrate => {
            let pool = sqlx::PgPool::connect(&config.database_url).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            tracing::info!("migrations applied");
        }
        Command::Seed => {
            let pool = sqlx::PgPool::connect(&config.database_url).await?;
            seed::run(&pool, &config.seed_dir).await?;
        }
        Command::Serve => {
            let bind = config.bind.clone();
            let state = AppState::connect(config).await?;
            // Migrations run here too so a single-container dev run needs no orchestration;
            // in compose the dedicated migrate job has already done the work and this is a
            // no-op.
            sqlx::migrate!("./migrations").run(state.db()).await?;

            let app = routes::router(state);
            let listener = tokio::net::TcpListener::bind(&bind).await?;
            tracing::info!(%bind, "pqcas-api listening");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
    }

    Ok(())
}

/// Let compose/systemd stop us cleanly: finish in-flight requests, then exit.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
