use crate::card::Card;
use crate::ids::{GameId, PlayerId};
use serde::{Deserialize, Serialize};

/// All domain events the Game aggregate can emit.
/// Collected in-memory, drained by the application layer, then published.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    GameStarted {
        game_id: GameId,
    },
    CardDealt {
        game_id: GameId,
        /// None means the card went to the dealer.
        player_id: Option<PlayerId>,
        card: Card,
    },
    PlayerStood {
        game_id: GameId,
        player_id: PlayerId,
    },
    PlayerBust {
        game_id: GameId,
        player_id: PlayerId,
    },
    GameEnded {
        game_id: GameId,
        dealer_score: u8,
    },
}

impl DomainEvent {
    pub fn game_id(&self) -> GameId {
        match self {
            DomainEvent::GameStarted { game_id }
            | DomainEvent::CardDealt { game_id, .. }
            | DomainEvent::PlayerStood { game_id, .. }
            | DomainEvent::PlayerBust { game_id, .. }
            | DomainEvent::GameEnded { game_id, .. } => *game_id,
        }
    }
}
