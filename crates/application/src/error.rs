use domain::error::DomainError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::PlayerNotFound(m) | DomainError::GameNotFound(m) => {
                AppError::NotFound(m)
            }
            DomainError::GameAlreadyStarted => {
                AppError::Conflict("game already started".into())
            }
            DomainError::NotPlayerTurn => AppError::Conflict("not your turn".into()),
            DomainError::GameNotInProgress => {
                AppError::Conflict("game not in progress".into())
            }
            DomainError::InvalidBet(m) | DomainError::InvalidAction(m) => {
                AppError::InvalidInput(m)
            }
            DomainError::InsufficientChips { required, available } => {
                AppError::InvalidInput(format!(
                    "insufficient chips: need {required}, have {available}"
                ))
            }
            DomainError::Repository(m) => AppError::Internal(m),
        }
    }
}
