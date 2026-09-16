pub mod catalog;
pub mod cavp;
pub mod health;
pub mod kem;
pub mod sse;
pub mod subtasks;
pub mod tests;

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let request_timeout = state.config().request_timeout();

    let api = Router::new()
        .route("/algorithms", get(catalog::list_algorithms))
        .route(
            "/endpoints",
            get(catalog::list_endpoints).post(catalog::create_endpoint),
        )
        .route("/tests", get(tests::list_tests).post(tests::create_test))
        .route("/tests/{id}", get(tests::get_test))
        .route("/tests/{id}/matrix", get(tests::get_matrix))
        .route("/subtasks/{id}", get(subtasks::get_subtask))
        .route("/kem/runs", get(kem::list_runs).post(kem::create_run))
        .route("/kem/runs/{id}", get(kem::get_run))
        .route("/cavp/packs", get(cavp::list_packs))
        .route("/cavp/packs/{slug}", get(cavp::get_pack))
        .route("/cavp/sessions", post(cavp::create_session))
        .route("/cavp/sessions/{id}", get(cavp::get_session))
        .route("/cavp/sessions/{id}/validate", post(cavp::validate_session))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            request_timeout,
        ));

    // Event streams deliberately skip both layers above: they are meant to stay open (a
    // timeout would cut every live matrix off mid-run) and compression would buffer the
    // very updates they exist to deliver.
    let live = Router::new()
        .route("/tests/{id}/events", get(tests::events))
        .route("/kem/runs/{id}/events", get(kem::events));

    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(health::metrics))
        .nest("/api/v1", api.merge(live))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
