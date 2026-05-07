use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid bet: {0}")]
    InvalidBet(String),

    #[error("invalid action: {0}")]
    InvalidAction(String),

    #[error("player not found: {0}")]
    PlayerNotFound(String),

    #[error("game not found: {0}")]
    GameNotFound(String),

    #[error("insufficient chips: need {required}, have {available}")]
    InsufficientChips { required: u32, available: u32 },

    #[error("game already started")]
    GameAlreadyStarted,

    #[error("game is not in progress")]
    GameNotInProgress,

    #[error("not player's turn")]
    NotPlayerTurn,

    #[error("repository error: {0}")]
    Repository(String),
}
