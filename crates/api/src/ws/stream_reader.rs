use std::sync::Arc;
use std::time::Duration;

use redis::{aio::ConnectionManager, streams::StreamReadOptions, AsyncCommands};
use tracing::{debug, error, warn};

use super::broadcaster::{BroadcastMsg, Broadcaster};

const STREAM_KEY: &str = "bj:events";
const BLOCK_MS: usize = 2000;
const BATCH: usize = 50;

/// Spawns a background Tokio task that reads from `bj:events` Redis Stream
/// and broadcasts each message to all connected WebSocket sessions.
///
/// Uses XREAD BLOCK so it sleeps in Redis until new events arrive.
pub fn spawn(conn: ConnectionManager, broadcaster: Arc<Broadcaster>) {
    tokio::spawn(async move {
        run(conn, broadcaster).await;
    });
}

async fn run(mut conn: ConnectionManager, broadcaster: Arc<Broadcaster>) {
    // "$" = start from the latest message (don't replay history on restart)
    let mut last_id = "$".to_string();

    loop {
        match read_batch(&mut conn, &last_id).await {
            Ok(None) => {
                // BLOCK timeout — no new events, loop again
            }
            Ok(Some(entries)) => {
                for (id, game_id, payload) in entries {
                    debug!(stream_id = %id, game_id = %game_id, "broadcasting event");
                    broadcaster.publish(BroadcastMsg {
                        game_id,
                        payload: payload.into(),
                    });
                    last_id = id;
                }
            }
            Err(e) => {
                error!(err = %e, "Redis stream read error — retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Returns `None` on BLOCK timeout, `Some(vec)` on messages.
async fn read_batch(
    conn: &mut ConnectionManager,
    last_id: &str,
) -> anyhow::Result<Option<Vec<(String, String, String)>>> {
    let opts = StreamReadOptions::default()
        .block(BLOCK_MS)
        .count(BATCH);

    let reply: redis::streams::StreamReadReply =
        conn.xread_options(&[STREAM_KEY], &[last_id], &opts).await?;

    if reply.keys.is_empty() {
        return Ok(None);
    }

    let mut out = Vec::new();
    for stream_key in reply.keys {
        for entry in stream_key.ids {
            let stream_id = entry.id.clone();
            let game_id = entry
                .map
                .get("game_id")
                .and_then(|v| redis_value_to_string(v))
                .unwrap_or_default();
            let payload = entry
                .map
                .get("payload")
                .and_then(|v| redis_value_to_string(v))
                .unwrap_or_default();

            if payload.is_empty() {
                warn!(stream_id, "stream entry missing payload, skipping");
                continue;
            }
            out.push((stream_id, game_id, payload));
        }
    }

    Ok(Some(out))
}

fn redis_value_to_string(v: &redis::Value) -> Option<String> {
    redis::from_redis_value(v).ok()
}
