use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use domain::ids::GameId;
use futures_util::StreamExt;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::{app_state::AppState, metrics::AppMetrics};

use super::broadcaster::Broadcaster;

/// `GET /ws/games/:id`
///
/// Upgrades to WebSocket and streams all DomainEvents for the specified game
/// as JSON text frames. Clients can send Ping; the server replies with Pong.
pub async fn ws_game(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let game_id = GameId::from_uuid(path.into_inner()).to_string();
    let broadcaster = state.broadcaster.clone();
    let metrics = state.metrics.clone();

    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    actix_web::rt::spawn(session_loop(session, msg_stream, game_id, broadcaster, metrics));

    Ok(response)
}

async fn session_loop(
    mut session: actix_ws::Session,
    mut msg_stream: actix_ws::MessageStream,
    game_id: String,
    broadcaster: Arc<Broadcaster>,
    metrics: Arc<AppMetrics>,
) {
    let mut rx = broadcaster.subscribe();
    metrics.ws_connections.add(1, &[]);

    tracing::debug!(game_id, "WebSocket session opened");

    loop {
        tokio::select! {
            // ── inbound from Redis broadcast ──────────────────────────────
            result = rx.recv() => {
                match result {
                    Ok(msg) if msg.game_id == game_id => {
                        if session.text(msg.payload.as_ref()).await.is_err() {
                            break; // client disconnected
                        }
                    }
                    Ok(_) => {} // different game — ignore
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!(game_id, dropped = n, "WS client lagged, events dropped");
                    }
                    Err(RecvError::Closed) => break,
                }
            }

            // ── inbound from WebSocket client ─────────────────────────────
            msg = msg_stream.next() => {
                match msg {
                    Some(Ok(Message::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(reason))) => {
                        let _ = session.close(reason).await;
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(err = %e, "WS protocol error");
                        break;
                    }
                    None => break, // stream exhausted
                    _ => {}        // Text/Binary from client — ignored
                }
            }
        }
    }

    metrics.ws_connections.add(-1, &[]);
    tracing::debug!(game_id, "WebSocket session closed");
}
