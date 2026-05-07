use crate::bet::Chips;
use crate::error::DomainError;
use crate::ids::PlayerId;
use serde::{Deserialize, Serialize};

/// Entity — identified by PlayerId, mutable balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    id: PlayerId,
    name: String,
    balance: Chips,
}

impl Player {
    pub fn new(id: PlayerId, name: impl Into<String>, initial_balance: u32) -> Self {
        Self {
            id,
            name: name.into(),
            balance: Chips::new(initial_balance),
        }
    }

    pub fn id(&self) -> PlayerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn balance(&self) -> Chips {
        self.balance
    }

    /// Deduct chips — fails if balance is insufficient.
    pub fn deduct(&mut self, amount: Chips) -> Result<(), DomainError> {
        self.balance = self
            .balance
            .checked_sub(amount)
            .ok_or(DomainError::InsufficientChips {
                required: amount.amount(),
                available: self.balance.amount(),
            })?;
        Ok(())
    }

    /// Credit chips back to the player (win or refund).
    pub fn credit(&mut self, amount: Chips) {
        self.balance = self.balance + amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> Player {
        Player::new(PlayerId::new(), "Alice", 500)
    }

    #[test]
    fn deduct_within_balance() {
        let mut p = player();
        p.deduct(Chips::new(200)).unwrap();
        assert_eq!(p.balance().amount(), 300);
    }

    #[test]
    fn deduct_overdraft_is_err() {
        let mut p = player();
        assert!(p.deduct(Chips::new(600)).is_err());
    }

    #[test]
    fn credit_increases_balance() {
        let mut p = player();
        p.credit(Chips::new(100));
        assert_eq!(p.balance().amount(), 600);
    }
}
