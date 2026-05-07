use domain::ids::{GameId, PlayerId};

#[derive(Debug)]
pub struct CreatePlayerCommand {
    pub name: String,
    pub initial_balance: u32,
}

#[derive(Debug)]
pub struct CreateGameCommand;

#[derive(Debug)]
pub struct JoinGameCommand {
    pub game_id: GameId,
    pub player_id: PlayerId,
}

#[derive(Debug)]
pub struct OpenBettingCommand {
    pub game_id: GameId,
}

#[derive(Debug)]
pub struct PlaceBetCommand {
    pub game_id: GameId,
    pub player_id: PlayerId,
    pub amount: u32,
}

#[derive(Debug)]
pub struct DealCommand {
    pub game_id: GameId,
}

#[derive(Debug)]
pub struct HitCommand {
    pub game_id: GameId,
    pub player_id: PlayerId,
}

#[derive(Debug)]
pub struct StandCommand {
    pub game_id: GameId,
    pub player_id: PlayerId,
}

#[derive(Debug)]
pub struct PlayDealerCommand {
    pub game_id: GameId,
}

#[derive(Debug)]
pub struct SettleCommand {
    pub game_id: GameId,
}
