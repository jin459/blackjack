use crate::error::DomainError;
use crate::events::DomainEvent;
use crate::game::Game;
use crate::ids::{GameId, PlayerId};
use crate::player::Player;
use async_trait::async_trait;

// ---------------------------------------------------------------------------
// Ports — infrastructure implements these, domain depends only on the trait.
// ---------------------------------------------------------------------------

/// Persistence port for Game aggregates.
#[async_trait]
pub trait GameRepository: Send + Sync {
    async fn save(&self, game: &Game) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: GameId) -> Result<Option<Game>, DomainError>;
    /// All games not yet Settled — used by the lobby.
    async fn list_active(&self) -> Result<Vec<Game>, DomainError>;
    async fn delete(&self, id: GameId) -> Result<(), DomainError>;
}

/// Persistence port for Player entities.
#[async_trait]
pub trait PlayerRepository: Send + Sync {
    async fn save(&self, player: &Player) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: PlayerId) -> Result<Option<Player>, DomainError>;
    async fn find_all(&self) -> Result<Vec<Player>, DomainError>;
    async fn delete(&self, id: PlayerId) -> Result<(), DomainError>;
}

/// Messaging port — publishes domain events to the message queue.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish_all(&self, events: Vec<DomainEvent>) -> Result<(), DomainError>;
}
