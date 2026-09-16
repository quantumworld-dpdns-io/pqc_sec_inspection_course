//! Redis Streams consumer-group plumbing.
//!
//! Streams (not plain lists) because a consumer group gives us at-least-once delivery plus
//! `XAUTOCLAIM`: if a worker dies holding a probe, another one picks the job back up
//! instead of leaving a subtask stuck on "running" forever.

use redis::aio::ConnectionManager;
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::AsyncCommands;

use pqcas_domain::jobs::CONSUMER_GROUP;

pub struct Delivery {
    pub stream: String,
    pub id: String,
    pub payload: String,
}

/// Create the consumer group, tolerating the BUSYGROUP error that means "already there".
pub async fn ensure_group(conn: &mut ConnectionManager, stream: &str) -> anyhow::Result<()> {
    let result: redis::RedisResult<String> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(CONSUMER_GROUP)
        .arg("$")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(err) if err.to_string().contains("BUSYGROUP") => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Block for new entries on the given streams.
pub async fn read(
    conn: &mut ConnectionManager,
    streams: &[&str],
    consumer: &str,
    block_ms: usize,
    count: usize,
) -> anyhow::Result<Vec<Delivery>> {
    let options = StreamReadOptions::default()
        .group(CONSUMER_GROUP, consumer)
        .block(block_ms)
        .count(count);

    let ids: Vec<&str> = streams.iter().map(|_| ">").collect();
    let reply: Option<StreamReadReply> = conn.xread_options(streams, &ids, &options).await?;

    Ok(collect(reply))
}

/// Take over entries that have been pending on a dead consumer for too long.
pub async fn autoclaim(
    conn: &mut ConnectionManager,
    stream: &str,
    consumer: &str,
    min_idle_ms: u64,
) -> anyhow::Result<Vec<Delivery>> {
    let reply: redis::Value = redis::cmd("XAUTOCLAIM")
        .arg(stream)
        .arg(CONSUMER_GROUP)
        .arg(consumer)
        .arg(min_idle_ms)
        .arg("0-0")
        .arg("COUNT")
        .arg(16)
        .query_async(conn)
        .await?;

    // XAUTOCLAIM replies [next_cursor, [[id, [field, value, ...]], ...], [deleted...]].
    let mut out = Vec::new();
    if let redis::Value::Array(parts) = reply {
        if let Some(redis::Value::Array(entries)) = parts.get(1) {
            for entry in entries {
                if let redis::Value::Array(pair) = entry {
                    let id = match pair.first() {
                        Some(value) => redis::from_redis_value::<String>(value).unwrap_or_default(),
                        None => continue,
                    };
                    let fields = match pair.get(1) {
                        Some(value) => {
                            redis::from_redis_value::<Vec<String>>(value).unwrap_or_default()
                        }
                        None => continue,
                    };
                    if let Some(payload) = field_value(&fields, "job") {
                        out.push(Delivery {
                            stream: stream.to_string(),
                            id,
                            payload,
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}

pub async fn ack(conn: &mut ConnectionManager, stream: &str, id: &str) -> anyhow::Result<()> {
    let _: i64 = conn.xack(stream, CONSUMER_GROUP, &[id]).await?;
    Ok(())
}

fn collect(reply: Option<StreamReadReply>) -> Vec<Delivery> {
    let mut out = Vec::new();
    let Some(reply) = reply else { return out };
    for key in reply.keys {
        for entry in key.ids {
            if let Some(payload) = entry
                .map
                .get("job")
                .and_then(|v| redis::from_redis_value::<String>(v).ok())
            {
                out.push(Delivery {
                    stream: key.key.clone(),
                    id: entry.id.clone(),
                    payload,
                });
            }
        }
    }
    out
}

fn field_value(fields: &[String], name: &str) -> Option<String> {
    fields
        .chunks(2)
        .find(|pair| pair.first().map(String::as_str) == Some(name))
        .and_then(|pair| pair.get(1).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_lookup_walks_flat_pairs() {
        let fields = vec!["other".into(), "x".into(), "job".into(), "{}".into()];
        assert_eq!(field_value(&fields, "job").as_deref(), Some("{}"));
        assert_eq!(field_value(&fields, "missing"), None);
    }
}
