use crate::bet::Bet;
use crate::card::{Card, Rank, Suit};
use crate::error::DomainError;
use crate::events::DomainEvent;
use crate::hand::Hand;
use crate::ids::{GameId, PlayerId};
use serde::{Deserialize, Serialize};

const MAX_SEATS: usize = 7;
const SHOE_DECKS: usize = 6;
const DEALER_STAND_ON: u8 = 17;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    WaitingForPlayers,
    WaitingForBets,
    PlayerTurn { seat_index: usize },
    DealerTurn,
    Settled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatStatus {
    WaitingBet,
    Playing,
    Stood,
    Bust,
    Blackjack,
}

/// One player's position at the table for a single round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub player_id: PlayerId,
    pub hand: Hand,
    pub bet: Option<Bet>,
    pub status: SeatStatus,
}

impl Seat {
    fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            hand: Hand::new(),
            bet: None,
            status: SeatStatus::WaitingBet,
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregate root
// ---------------------------------------------------------------------------

/// Game is the aggregate root — the only entry point for mutating game state.
/// All invariants are enforced here. Domain events are collected and must be
/// drained by the application layer after each command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    id: GameId,
    status: GameStatus,
    shoe: Vec<Card>,
    dealer_hand: Hand,
    seats: Vec<Seat>,
    #[serde(skip)]
    pending_events: Vec<DomainEvent>,
}

impl Game {
    pub fn new(id: GameId) -> Self {
        Self {
            id,
            status: GameStatus::WaitingForPlayers,
            shoe: Self::build_shoe(SHOE_DECKS),
            dealer_hand: Hand::new(),
            seats: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    // --- Accessors ---

    pub fn id(&self) -> GameId {
        self.id
    }

    pub fn status(&self) -> &GameStatus {
        &self.status
    }

    pub fn dealer_hand(&self) -> &Hand {
        &self.dealer_hand
    }

    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn seat_for(&self, player_id: PlayerId) -> Option<&Seat> {
        self.seats.iter().find(|s| s.player_id == player_id)
    }

    /// Takes all pending domain events — call once after every command.
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }

    // --- Commands ---

    pub fn join(&mut self, player_id: PlayerId) -> Result<(), DomainError> {
        if !matches!(self.status, GameStatus::WaitingForPlayers) {
            return Err(DomainError::InvalidAction(
                "cannot join after game has started".into(),
            ));
        }
        if self.seats.len() >= MAX_SEATS {
            return Err(DomainError::InvalidAction(
                format!("table is full (max {MAX_SEATS})"),
            ));
        }
        if self.seats.iter().any(|s| s.player_id == player_id) {
            return Err(DomainError::InvalidAction("player already seated".into()));
        }
        self.seats.push(Seat::new(player_id));
        Ok(())
    }

    pub fn open_betting(&mut self) -> Result<(), DomainError> {
        if !matches!(self.status, GameStatus::WaitingForPlayers) {
            return Err(DomainError::InvalidAction(
                "can only open betting from WaitingForPlayers".into(),
            ));
        }
        if self.seats.is_empty() {
            return Err(DomainError::InvalidAction(
                "need at least one player".into(),
            ));
        }
        self.status = GameStatus::WaitingForBets;
        Ok(())
    }

    pub fn place_bet(&mut self, player_id: PlayerId, bet: Bet) -> Result<(), DomainError> {
        if !matches!(self.status, GameStatus::WaitingForBets) {
            return Err(DomainError::InvalidAction("betting is not open".into()));
        }
        let seat = self
            .seats
            .iter_mut()
            .find(|s| s.player_id == player_id)
            .ok_or_else(|| DomainError::PlayerNotFound(player_id.to_string()))?;

        seat.bet = Some(bet);
        Ok(())
    }

    /// Deal 2 cards to each player and 2 to the dealer (interleaved).
    pub fn deal(&mut self) -> Result<(), DomainError> {
        if !matches!(self.status, GameStatus::WaitingForBets) {
            return Err(DomainError::InvalidAction("not in betting phase".into()));
        }
        if !self.seats.iter().all(|s| s.bet.is_some()) {
            return Err(DomainError::InvalidAction(
                "all players must place a bet before dealing".into(),
            ));
        }

        for _ in 0..2 {
            for i in 0..self.seats.len() {
                let card = self.draw()?;
                self.seats[i].hand.add(card);
                let pid = self.seats[i].player_id;
                self.pending_events.push(DomainEvent::CardDealt {
                    game_id: self.id,
                    player_id: Some(pid),
                    card,
                });
            }
            let card = self.draw()?;
            self.dealer_hand.add(card);
            self.pending_events.push(DomainEvent::CardDealt {
                game_id: self.id,
                player_id: None,
                card,
            });
        }

        // Classify initial hands
        for i in 0..self.seats.len() {
            self.seats[i].status = if self.seats[i].hand.is_blackjack() {
                SeatStatus::Blackjack
            } else {
                SeatStatus::Playing
            };
        }

        self.pending_events
            .push(DomainEvent::GameStarted { game_id: self.id });

        self.status = GameStatus::PlayerTurn { seat_index: 0 };
        self.advance_turn();

        Ok(())
    }

    pub fn hit(&mut self, player_id: PlayerId) -> Result<(), DomainError> {
        let idx = self.active_seat_index(player_id)?;
        let card = self.draw()?;
        self.seats[idx].hand.add(card);
        self.pending_events.push(DomainEvent::CardDealt {
            game_id: self.id,
            player_id: Some(player_id),
            card,
        });

        if self.seats[idx].hand.is_bust() {
            self.seats[idx].status = SeatStatus::Bust;
            self.pending_events.push(DomainEvent::PlayerBust {
                game_id: self.id,
                player_id,
            });
            self.status = GameStatus::PlayerTurn { seat_index: idx + 1 };
            self.advance_turn();
        }

        Ok(())
    }

    pub fn stand(&mut self, player_id: PlayerId) -> Result<(), DomainError> {
        let idx = self.active_seat_index(player_id)?;
        self.seats[idx].status = SeatStatus::Stood;
        self.pending_events.push(DomainEvent::PlayerStood {
            game_id: self.id,
            player_id,
        });
        self.status = GameStatus::PlayerTurn { seat_index: idx + 1 };
        self.advance_turn();
        Ok(())
    }

    /// Dealer draws until score >= DEALER_STAND_ON. Returns final score.
    pub fn play_dealer(&mut self) -> Result<u8, DomainError> {
        if !matches!(self.status, GameStatus::DealerTurn) {
            return Err(DomainError::InvalidAction("not dealer's turn".into()));
        }
        while self.dealer_hand.score() < DEALER_STAND_ON {
            let card = self.draw()?;
            self.dealer_hand.add(card);
            self.pending_events.push(DomainEvent::CardDealt {
                game_id: self.id,
                player_id: None,
                card,
            });
        }
        let score = self.dealer_hand.score();
        self.status = GameStatus::Settled;
        self.pending_events
            .push(DomainEvent::GameEnded { game_id: self.id, dealer_score: score });
        Ok(score)
    }

    // --- Internals ---

    fn draw(&mut self) -> Result<Card, DomainError> {
        self.shoe
            .pop()
            .ok_or_else(|| DomainError::InvalidAction("shoe is exhausted".into()))
    }

    fn active_seat_index(&self, player_id: PlayerId) -> Result<usize, DomainError> {
        let GameStatus::PlayerTurn { seat_index: idx } = self.status else {
            return Err(DomainError::InvalidAction("not in player turn phase".into()));
        };
        if idx >= self.seats.len() {
            return Err(DomainError::InvalidAction("no active seat".into()));
        }
        if self.seats[idx].player_id != player_id {
            return Err(DomainError::NotPlayerTurn);
        }
        if !matches!(self.seats[idx].status, SeatStatus::Playing) {
            return Err(DomainError::InvalidAction(
                "seat is not in playing state".into(),
            ));
        }
        Ok(idx)
    }

    /// Skip seats that are no longer Playing; transition to DealerTurn when done.
    fn advance_turn(&mut self) {
        loop {
            let GameStatus::PlayerTurn { seat_index: idx } = self.status else {
                break;
            };
            if idx >= self.seats.len() {
                self.status = GameStatus::DealerTurn;
                break;
            }
            if matches!(self.seats[idx].status, SeatStatus::Playing) {
                break;
            }
            self.status = GameStatus::PlayerTurn { seat_index: idx + 1 };
        }
    }

    fn build_shoe(num_decks: usize) -> Vec<Card> {
        let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
        let mut shoe = Vec::with_capacity(52 * num_decks);
        for _ in 0..num_decks {
            for suit in suits {
                for rank in Rank::all() {
                    shoe.push(Card::new(suit, rank));
                }
            }
        }
        // XorShift64 shuffle seeded from a random UUID
        let mut s = uuid::Uuid::new_v4().as_u128() as u64 | 1;
        for i in (1..shoe.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s as usize) % (i + 1);
            shoe.swap(i, j);
        }
        shoe
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bet::Bet;

    fn one_player_game() -> (Game, PlayerId) {
        let mut game = Game::new(GameId::new());
        let pid = PlayerId::new();
        game.join(pid).unwrap();
        game.open_betting().unwrap();
        game.place_bet(pid, Bet::new(100).unwrap()).unwrap();
        (game, pid)
    }

    #[test]
    fn deal_moves_to_player_turn() {
        let (mut g, _) = one_player_game();
        g.deal().unwrap();
        assert!(matches!(g.status(), GameStatus::PlayerTurn { seat_index: 0 }));
    }

    #[test]
    fn stand_with_single_player_moves_to_dealer() {
        let (mut g, pid) = one_player_game();
        g.deal().unwrap();
        // Only act if player isn't already blackjack
        if matches!(g.status(), GameStatus::PlayerTurn { .. }) {
            g.stand(pid).unwrap();
        }
        assert!(matches!(g.status(), GameStatus::DealerTurn));
    }

    #[test]
    fn wrong_player_cannot_act() {
        let (mut g, _) = one_player_game();
        g.deal().unwrap();
        assert!(g.hit(PlayerId::new()).is_err());
    }

    #[test]
    fn full_round_reaches_settled() {
        let (mut g, pid) = one_player_game();
        g.deal().unwrap();
        if matches!(g.status(), GameStatus::PlayerTurn { .. }) {
            g.stand(pid).unwrap();
        }
        if matches!(g.status(), GameStatus::DealerTurn) {
            g.play_dealer().unwrap();
        }
        assert!(matches!(g.status(), GameStatus::Settled));
    }

    #[test]
    fn events_are_emitted_and_drained() {
        let (mut g, _) = one_player_game();
        g.deal().unwrap();
        let events = g.drain_events();
        assert!(!events.is_empty());
        assert!(g.drain_events().is_empty()); // drained once = empty
    }

    #[test]
    fn table_max_7_players() {
        let mut g = Game::new(GameId::new());
        for _ in 0..MAX_SEATS {
            g.join(PlayerId::new()).unwrap();
        }
        assert!(g.join(PlayerId::new()).is_err());
    }
}
