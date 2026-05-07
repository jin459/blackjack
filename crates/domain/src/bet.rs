use crate::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

/// Value object — chip balance. Never negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Chips(u32);

impl Chips {
    pub const ZERO: Chips = Chips(0);

    pub fn new(amount: u32) -> Self {
        Self(amount)
    }

    pub fn amount(self) -> u32 {
        self.0
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

impl Add for Chips {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Chips {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl fmt::Display for Chips {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}c", self.0)
    }
}

/// Value object — an amount wagered on a round. Always > 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bet(Chips);

impl Bet {
    pub fn new(amount: u32) -> Result<Self, DomainError> {
        if amount == 0 {
            return Err(DomainError::InvalidBet(
                "bet must be greater than zero".into(),
            ));
        }
        Ok(Self(Chips::new(amount)))
    }

    pub fn chips(self) -> Chips {
        self.0
    }

    pub fn amount(self) -> u32 {
        self.0.amount()
    }

    /// Returns a new Bet with amount doubled (double-down).
    pub fn doubled(self) -> Self {
        Self(Chips::new(self.0.amount() * 2))
    }
}

impl fmt::Display for Bet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bet({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bet_is_rejected() {
        assert!(Bet::new(0).is_err());
    }

    #[test]
    fn double_bet() {
        let b = Bet::new(50).unwrap();
        assert_eq!(b.doubled().amount(), 100);
    }

    #[test]
    fn chips_saturate_on_overflow() {
        let big = Chips::new(u32::MAX);
        assert_eq!((big + Chips::new(1)).amount(), u32::MAX);
    }
}
