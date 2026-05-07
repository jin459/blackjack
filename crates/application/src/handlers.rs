use std::sync::Arc;
use tracing::instrument;

use domain::{
    bet::{Bet, Chips},
    game::Game,
    ids::{GameId, PlayerId},
    player::Player,
    repository::{EventPublisher, GameRepository, PlayerRepository},
    services::BlackjackRules,
};

use crate::{
    commands::*,
    error::AppError,
    queries::{
        game_to_state_dto, game_to_summary_dto, payout_to_dto, player_to_dto, GameStateDto,
        GameSummaryDto, GetGameStateQuery, GetPlayerQuery, ListActiveGamesQuery, PayoutDto,
        PlayerDto,
    },
};

// ---------------------------------------------------------------------------
// GameCommandService — write side (CQRS)
// ---------------------------------------------------------------------------

pub struct GameCommandService {
    game_repo: Arc<dyn GameRepository>,
    player_repo: Arc<dyn PlayerRepository>,
    event_pub: Arc<dyn EventPublisher>,
}

impl GameCommandService {
    pub fn new(
        game_repo: Arc<dyn GameRepository>,
        player_repo: Arc<dyn PlayerRepository>,
        event_pub: Arc<dyn EventPublisher>,
    ) -> Self {
        Self { game_repo, player_repo, event_pub }
    }

    #[instrument(skip(self), fields(player.name = %cmd.name))]
    pub async fn create_player(&self, cmd: CreatePlayerCommand) -> Result<PlayerDto, AppError> {
        let id = PlayerId::new();
        let player = Player::new(id, &cmd.name, cmd.initial_balance);
        self.player_repo.save(&player).await?;
        tracing::info!(player.id = %id, "player created");
        Ok(player_to_dto(&player))
    }

    #[instrument(skip(self))]
    pub async fn create_game(&self, _cmd: CreateGameCommand) -> Result<String, AppError> {
        let id = GameId::new();
        let game = Game::new(id);
        self.game_repo.save(&game).await?;
        tracing::info!(game.id = %id, "game created");
        Ok(id.to_string())
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id, player.id = %cmd.player_id))]
    pub async fn join_game(&self, cmd: JoinGameCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        self.load_player(cmd.player_id).await?; // verify player exists
        game.join(cmd.player_id)?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id))]
    pub async fn open_betting(&self, cmd: OpenBettingCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        game.open_betting()?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id, player.id = %cmd.player_id, bet = cmd.amount))]
    pub async fn place_bet(&self, cmd: PlaceBetCommand) -> Result<(), AppError> {
        let bet = Bet::new(cmd.amount)?;

        let mut player = self.load_player(cmd.player_id).await?;
        player.deduct(bet.chips())?;

        let mut game = self.load_game(cmd.game_id).await?;
        game.place_bet(cmd.player_id, bet)?;

        self.player_repo.save(&player).await?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id))]
    pub async fn deal(&self, cmd: DealCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        game.deal()?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id, player.id = %cmd.player_id))]
    pub async fn hit(&self, cmd: HitCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        game.hit(cmd.player_id)?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id, player.id = %cmd.player_id))]
    pub async fn stand(&self, cmd: StandCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        game.stand(cmd.player_id)?;
        self.save_and_publish(&mut game).await
    }

    #[instrument(skip(self), fields(game.id = %cmd.game_id))]
    pub async fn play_dealer(&self, cmd: PlayDealerCommand) -> Result<(), AppError> {
        let mut game = self.load_game(cmd.game_id).await?;
        game.play_dealer()?;
        self.save_and_publish(&mut game).await
    }

    /// Calculate payouts, credit winners, and return results.
    #[instrument(skip(self), fields(game.id = %cmd.game_id))]
    pub async fn settle(&self, cmd: SettleCommand) -> Result<Vec<PayoutDto>, AppError> {
        let game = self.load_game(cmd.game_id).await?;
        let payouts = BlackjackRules::calculate_payouts(&game);

        for payout in &payouts {
            if payout.return_amount > 0 {
                let mut player = self.load_player(payout.player_id).await?;
                player.credit(Chips::new(payout.return_amount));
                self.player_repo.save(&player).await?;
                tracing::info!(
                    player.id = %payout.player_id,
                    outcome = ?payout.outcome,
                    returned = payout.return_amount,
                    "payout applied"
                );
            }
        }

        Ok(payouts.iter().map(payout_to_dto).collect())
    }

    // --- Helpers ---

    async fn load_game(&self, id: GameId) -> Result<Game, AppError> {
        self.game_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("game {id}")))
    }

    async fn load_player(&self, id: PlayerId) -> Result<Player, AppError> {
        self.player_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("player {id}")))
    }

    async fn save_and_publish(&self, game: &mut Game) -> Result<(), AppError> {
        let events = game.drain_events();
        self.game_repo.save(game).await?;
        if !events.is_empty() {
            self.event_pub.publish_all(events).await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GameQueryService — read side (CQRS)
// ---------------------------------------------------------------------------

pub struct GameQueryService {
    game_repo: Arc<dyn GameRepository>,
    player_repo: Arc<dyn PlayerRepository>,
}

impl GameQueryService {
    pub fn new(
        game_repo: Arc<dyn GameRepository>,
        player_repo: Arc<dyn PlayerRepository>,
    ) -> Self {
        Self { game_repo, player_repo }
    }

    #[instrument(skip(self), fields(game.id = %q.game_id))]
    pub async fn get_game_state(&self, q: GetGameStateQuery) -> Result<GameStateDto, AppError> {
        let game = self
            .game_repo
            .find_by_id(q.game_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("game {}", q.game_id)))?;
        Ok(game_to_state_dto(&game))
    }

    #[instrument(skip(self), fields(player.id = %q.player_id))]
    pub async fn get_player(&self, q: GetPlayerQuery) -> Result<PlayerDto, AppError> {
        let player = self
            .player_repo
            .find_by_id(q.player_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("player {}", q.player_id)))?;
        Ok(player_to_dto(&player))
    }

    #[instrument(skip(self))]
    pub async fn list_active_games(
        &self,
        _q: ListActiveGamesQuery,
    ) -> Result<Vec<GameSummaryDto>, AppError> {
        let games = self.game_repo.list_active().await?;
        Ok(games.iter().map(game_to_summary_dto).collect())
    }
}
