use domain::ids::{GameId, PlayerId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Query inputs
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct GetGameStateQuery {
    pub game_id: GameId,
}

#[derive(Debug)]
pub struct GetPlayerQuery {
    pub player_id: PlayerId,
}

#[derive(Debug)]
pub struct ListActiveGamesQuery;

// ---------------------------------------------------------------------------
// DTOs — what the API layer receives (no domain types leak out)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDto {
    pub suit: String,
    pub rank: String,
    pub value: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandDto {
    pub cards: Vec<CardDto>,
    pub score: u8,
    pub outcome: String, // "hard", "soft", "blackjack", "bust"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeatDto {
    pub player_id: String,
    pub hand: HandDto,
    pub bet: Option<u32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateDto {
    pub id: String,
    pub status: String,
    pub dealer_hand: HandDto,
    pub seats: Vec<SeatDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummaryDto {
    pub id: String,
    pub status: String,
    pub seat_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerDto {
    pub id: String,
    pub name: String,
    pub balance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutDto {
    pub player_id: String,
    pub bet_amount: u32,
    pub outcome: String,
    pub return_amount: u32,
}

// ---------------------------------------------------------------------------
// Mapping helpers (domain → DTO)
// ---------------------------------------------------------------------------

use domain::{
    card::Card,
    game::{Game, GameStatus, SeatStatus},
    hand::{Hand, HandOutcome},
    player::Player,
    services::{Payout, RoundOutcome},
};

pub fn card_to_dto(c: &Card) -> CardDto {
    CardDto {
        suit: format!("{}", c.suit),
        rank: format!("{}", c.rank),
        value: c.base_value(),
    }
}

pub fn hand_to_dto(h: &Hand) -> HandDto {
    let outcome_str = match h.outcome() {
        HandOutcome::Soft(_) => "soft",
        HandOutcome::Hard(_) => "hard",
        HandOutcome::Blackjack => "blackjack",
        HandOutcome::Bust => "bust",
    };
    HandDto {
        cards: h.cards().iter().map(card_to_dto).collect(),
        score: h.score(),
        outcome: outcome_str.to_string(),
    }
}

fn seat_status_str(s: &SeatStatus) -> &'static str {
    match s {
        SeatStatus::WaitingBet => "waiting_bet",
        SeatStatus::Playing => "playing",
        SeatStatus::Stood => "stood",
        SeatStatus::Bust => "bust",
        SeatStatus::Blackjack => "blackjack",
    }
}

fn game_status_str(s: &GameStatus) -> String {
    match s {
        GameStatus::WaitingForPlayers => "waiting_for_players".into(),
        GameStatus::WaitingForBets => "waiting_for_bets".into(),
        GameStatus::PlayerTurn { seat_index } => format!("player_turn:{seat_index}"),
        GameStatus::DealerTurn => "dealer_turn".into(),
        GameStatus::Settled => "settled".into(),
    }
}

pub fn game_to_state_dto(g: &Game) -> GameStateDto {
    GameStateDto {
        id: g.id().to_string(),
        status: game_status_str(g.status()),
        dealer_hand: hand_to_dto(g.dealer_hand()),
        seats: g
            .seats()
            .iter()
            .map(|s| SeatDto {
                player_id: s.player_id.to_string(),
                hand: hand_to_dto(&s.hand),
                bet: s.bet.map(|b| b.amount()),
                status: seat_status_str(&s.status).to_string(),
            })
            .collect(),
    }
}

pub fn game_to_summary_dto(g: &Game) -> GameSummaryDto {
    GameSummaryDto {
        id: g.id().to_string(),
        status: game_status_str(g.status()),
        seat_count: g.seats().len(),
    }
}

pub fn player_to_dto(p: &Player) -> PlayerDto {
    PlayerDto {
        id: p.id().to_string(),
        name: p.name().to_string(),
        balance: p.balance().amount(),
    }
}

pub fn payout_to_dto(p: &Payout) -> PayoutDto {
    let outcome = match p.outcome {
        RoundOutcome::PlayerBlackjack => "blackjack",
        RoundOutcome::PlayerWin => "win",
        RoundOutcome::Push => "push",
        RoundOutcome::PlayerLose => "lose",
    };
    PayoutDto {
        player_id: p.player_id.to_string(),
        bet_amount: p.bet_amount,
        outcome: outcome.to_string(),
        return_amount: p.return_amount,
    }
}
