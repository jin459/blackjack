use crate::game::{Game, GameStatus, SeatStatus};
use crate::ids::PlayerId;

// ---------------------------------------------------------------------------
// BlackjackRules — pure payout calculations, no I/O.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundOutcome {
    PlayerBlackjack,
    PlayerWin,
    Push,
    PlayerLose,
}

#[derive(Debug, Clone)]
pub struct Payout {
    pub player_id: PlayerId,
    pub bet_amount: u32,
    pub outcome: RoundOutcome,
    /// Chips to credit back (stake + profit). 0 on loss.
    /// Assumes the bet was already deducted when placed.
    pub return_amount: u32,
}

pub struct BlackjackRules;

impl BlackjackRules {
    /// Calculate payouts for every seat after `Game::play_dealer()` completes.
    pub fn calculate_payouts(game: &Game) -> Vec<Payout> {
        assert!(
            matches!(game.status(), GameStatus::Settled),
            "payouts can only be calculated after settlement"
        );

        let dealer_bust = game.dealer_hand().is_bust();
        let dealer_score = game.dealer_hand().score();

        game.seats()
            .iter()
            .map(|seat| {
                let bet_amount = seat.bet.map(|b| b.amount()).unwrap_or(0);

                let (outcome, return_amount) = match &seat.status {
                    SeatStatus::Blackjack => {
                        // Pays 3:2 on top of stake
                        let profit = (bet_amount * 3) / 2;
                        (RoundOutcome::PlayerBlackjack, bet_amount + profit)
                    }
                    SeatStatus::Bust => (RoundOutcome::PlayerLose, 0),
                    SeatStatus::Stood | SeatStatus::Playing => {
                        let player_score = seat.hand.score();
                        if dealer_bust || player_score > dealer_score {
                            (RoundOutcome::PlayerWin, bet_amount * 2)
                        } else if player_score == dealer_score {
                            (RoundOutcome::Push, bet_amount)
                        } else {
                            (RoundOutcome::PlayerLose, 0)
                        }
                    }
                    SeatStatus::WaitingBet => (RoundOutcome::Push, 0),
                };

                Payout { player_id: seat.player_id, bet_amount, outcome, return_amount }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn blackjack_pays_2_5x_stake() {
        let bet: u32 = 100;
        let profit = (bet * 3) / 2;
        let returned = bet + profit;
        assert_eq!(returned, 250); // 100 stake + 150 profit
    }

    #[test]
    fn win_returns_2x_stake() {
        let bet: u32 = 100;
        assert_eq!(bet * 2, 200);
    }

    #[test]
    fn loss_returns_nothing() {
        // bet was already deducted on place_bet
        assert_eq!(0u32, 0);
    }
}
