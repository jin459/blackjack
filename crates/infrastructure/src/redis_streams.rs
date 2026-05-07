use async_trait::async_trait;
use domain::{error::DomainError, events::DomainEvent, repository::EventPublisher};
use redis::aio::ConnectionManager;
use tracing::instrument;

const STREAM_KEY: &str = "bj:events";

fn repo_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}

/// Publishes domain events to a Redis Stream (`bj:events`).
/// Consumers (websocket broadcaster, audit log) read from this stream.
pub struct RedisEventPublisher {
    conn: ConnectionManager,
}

impl RedisEventPublisher {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl EventPublisher for RedisEventPublisher {
    #[instrument(skip(self, events), fields(count = events.len()))]
    async fn publish_all(&self, events: Vec<DomainEvent>) -> Result<(), DomainError> {
        let mut c = self.conn.clone();

        for event in events {
            let event_type = event_type_name(&event);
            let game_id = event.game_id().to_string();
            let payload = serde_json::to_string(&event).map_err(repo_err)?;

            // XADD bj:events * event_type <name> game_id <id> payload <json>
            let stream_id: String = redis::cmd("XADD")
                .arg(STREAM_KEY)
                .arg("*")
                .arg("event_type")
                .arg(event_type)
                .arg("game_id")
                .arg(&game_id)
                .arg("payload")
                .arg(&payload)
                .query_async(&mut c)
                .await
                .map_err(repo_err)?;

            tracing::debug!(stream_id = %stream_id, event_type, "domain event published");
        }

        Ok(())
    }
}

fn event_type_name(e: &DomainEvent) -> &'static str {
    match e {
        DomainEvent::GameStarted { .. } => "game_started",
        DomainEvent::CardDealt { .. } => "card_dealt",
        DomainEvent::PlayerStood { .. } => "player_stood",
        DomainEvent::PlayerBust { .. } => "player_bust",
        DomainEvent::GameEnded { .. } => "game_ended",
    }
}
