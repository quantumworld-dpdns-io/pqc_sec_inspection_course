//! Shared Server-Sent Events plumbing.
//!
//! Live updates come from Redis pub/sub rather than polling Postgres, so 30 open browser
//! tabs cost one subscription each instead of 30 queries a second.

use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{Stream, StreamExt};

use crate::error::ApiError;
use crate::state::AppState;

pub async fn subscribe(
    state: &AppState,
    channel: String,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let client = state.pubsub_client().await.map_err(ApiError::Other)?;
    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(ApiError::Redis)?;
    pubsub.subscribe(&channel).await.map_err(ApiError::Redis)?;

    let stream = async_stream::stream! {
        let mut messages = pubsub.on_message();
        while let Some(msg) = messages.next().await {
            let payload: String = match msg.get_payload() {
                Ok(p) => p,
                Err(err) => {
                    tracing::warn!(%err, "dropping malformed pubsub payload");
                    continue;
                }
            };
            yield Ok(Event::default().data(payload));
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
