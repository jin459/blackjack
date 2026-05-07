use crate::card::Card;
use serde::{Deserialize, Serialize};

/// Describes the scored state of a hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandOutcome {
    /// Ace counted as 11 without busting.
    Soft(u8),
    /// No ace counted as 11.
    Hard(u8),
    /// First two cards sum to 21.
    Blackjack,
    /// Total exceeds 21.
    Bust,
}

impl HandOutcome {
    pub fn score(self) -> u8 {
        match self {
            HandOutcome::Soft(n) | HandOutcome::Hard(n) => n,
            HandOutcome::Blackjack => 21,
            HandOutcome::Bust => 0,
        }
    }

    pub fn is_bust(self) -> bool {
        matches!(self, HandOutcome::Bust)
    }

    pub fn is_blackjack(self) -> bool {
        matches!(self, HandOutcome::Blackjack)
    }
}

/// Value object — an ordered set of cards with blackjack scoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub fn new() -> Self {
        Self { cards: Vec::new() }
    }

    pub fn add(&mut self, card: Card) {
        self.cards.push(card);
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// True when the hand has exactly two cards of the same rank (eligible to split).
    pub fn is_pair(&self) -> bool {
        self.cards.len() == 2 && self.cards[0].rank == self.cards[1].rank
    }

    /// Raw numeric total — Ace reduced from 11 to 1 as needed to stay ≤ 21.
    pub fn score(&self) -> u8 {
        let mut total: u8 = 0;
        let mut soft_aces: u8 = 0;

        for card in &self.cards {
            if card.is_ace() {
                soft_aces += 1;
                total = total.saturating_add(11);
            } else {
                total = total.saturating_add(card.base_value());
            }
        }

        while total > 21 && soft_aces > 0 {
            total -= 10;
            soft_aces -= 1;
        }

        total
    }

    /// Full classified outcome.
    pub fn outcome(&self) -> HandOutcome {
        let score = self.score();

        if score > 21 {
            return HandOutcome::Bust;
        }

        // Natural blackjack: exactly two cards totalling 21
        if self.cards.len() == 2 && score == 21 {
            return HandOutcome::Blackjack;
        }

        if self.has_soft_ace() {
            HandOutcome::Soft(score)
        } else {
            HandOutcome::Hard(score)
        }
    }

    pub fn is_bust(&self) -> bool {
        self.outcome().is_bust()
    }

    pub fn is_blackjack(&self) -> bool {
        self.outcome().is_blackjack()
    }

    // True when at least one ace is still counted as 11 after adjustments.
    fn has_soft_ace(&self) -> bool {
        let mut total: u8 = 0;
        let mut soft_aces: u8 = 0;

        for card in &self.cards {
            if card.is_ace() {
                soft_aces += 1;
                total = total.saturating_add(11);
            } else {
                total = total.saturating_add(card.base_value());
            }
        }

        while total > 21 && soft_aces > 0 {
            total -= 10;
            soft_aces -= 1;
        }

        soft_aces > 0
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Rank, Suit};

    fn card(rank: Rank) -> Card {
        Card::new(Suit::Spades, rank)
    }

    #[test]
    fn ace_plus_king_is_blackjack() {
        let mut h = Hand::new();
        h.add(card(Rank::Ace));
        h.add(card(Rank::King));
        assert!(h.is_blackjack());
        assert_eq!(h.score(), 21);
    }

    #[test]
    fn ace_goes_soft_then_hard() {
        let mut h = Hand::new();
        h.add(card(Rank::Ace));
        h.add(card(Rank::Nine));
        assert_eq!(h.outcome(), HandOutcome::Soft(20));

        h.add(card(Rank::Five));
        // Ace must become 1: 1+9+5 = 15
        assert_eq!(h.score(), 15);
        assert!(matches!(h.outcome(), HandOutcome::Hard(15)));
    }

    #[test]
    fn bust_over_21() {
        let mut h = Hand::new();
        h.add(card(Rank::King));
        h.add(card(Rank::Queen));
        h.add(card(Rank::Five));
        assert!(h.is_bust());
    }

    #[test]
    fn pair_detection() {
        let mut h = Hand::new();
        h.add(card(Rank::Eight));
        h.add(card(Rank::Eight));
        assert!(h.is_pair());
    }
}
