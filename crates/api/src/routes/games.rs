use actix_web::{get, post, web, HttpResponse};
use application::{
    commands::{
        CreateGameCommand, DealCommand, HitCommand, JoinGameCommand, OpenBettingCommand,
        PlaceBetCommand, PlayDealerCommand, SettleCommand, StandCommand,
    },
    queries::{GetGameStateQuery, ListActiveGamesQuery},
};
use domain::ids::{GameId, PlayerId};
use serde::Deserialize;
use uuid::Uuid;

use crate::{app_state::AppState, error::ApiError};

// --- Request bodies ---

#[derive(Deserialize)]
pub struct JoinBody {
    pub player_id: Uuid,
}

#[derive(Deserialize)]
pub struct BetBody {
    pub player_id: Uuid,
    pub amount: u32,
}

#[derive(Deserialize)]
pub struct PlayerBody {
    pub player_id: Uuid,
}

// --- Handlers ---

#[post("/games")]
pub async fn create_game(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let id = state.cmd.create_game(CreateGameCommand).await?;

    state.metrics.games_created.add(1, &[]);
    state.metrics.active_games.add(1, &[]);

    Ok(HttpResponse::Created().json(serde_json::json!({ "id": id })))
}

#[get("/games")]
pub async fn list_games(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let games = state.qry.list_active_games(ListActiveGamesQuery).await?;
    Ok(HttpResponse::Ok().json(games))
}

#[get("/games/{id}")]
pub async fn get_game(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let dto = state
        .qry
        .get_game_state(GetGameStateQuery {
            game_id: GameId::from_uuid(path.into_inner()),
        })
        .await?;
    Ok(HttpResponse::Ok().json(dto))
}

#[post("/games/{id}/join")]
pub async fn join_game(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<JoinBody>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .join_game(JoinGameCommand {
            game_id: GameId::from_uuid(path.into_inner()),
            player_id: PlayerId::from_uuid(body.player_id),
        })
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/betting")]
pub async fn open_betting(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .open_betting(OpenBettingCommand {
            game_id: GameId::from_uuid(path.into_inner()),
        })
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/bet")]
pub async fn place_bet(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BetBody>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .place_bet(PlaceBetCommand {
            game_id: GameId::from_uuid(path.into_inner()),
            player_id: PlayerId::from_uuid(body.player_id),
            amount: body.amount,
        })
        .await?;

    state.metrics.bets_placed.add(1, &[]);
    state.metrics.chips_wagered.add(body.amount as u64, &[]);

    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/deal")]
pub async fn deal(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .deal(DealCommand {
            game_id: GameId::from_uuid(path.into_inner()),
        })
        .await?;

    // Initial deal: 2 cards × (players + dealer). We record a rough estimate
    // here; exact count is emitted as domain events on the stream.
    state.metrics.cards_dealt.add(4, &[]);

    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/hit")]
pub async fn hit(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<PlayerBody>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .hit(HitCommand {
            game_id: GameId::from_uuid(path.into_inner()),
            player_id: PlayerId::from_uuid(body.player_id),
        })
        .await?;

    state.metrics.cards_dealt.add(1, &[]);

    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/stand")]
pub async fn stand(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<PlayerBody>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .stand(StandCommand {
            game_id: GameId::from_uuid(path.into_inner()),
            player_id: PlayerId::from_uuid(body.player_id),
        })
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/dealer")]
pub async fn play_dealer(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    state
        .cmd
        .play_dealer(PlayDealerCommand {
            game_id: GameId::from_uuid(path.into_inner()),
        })
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/games/{id}/settle")]
pub async fn settle(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let payouts = state
        .cmd
        .settle(SettleCommand {
            game_id: GameId::from_uuid(path.into_inner()),
        })
        .await?;

    state.metrics.active_games.add(-1, &[]);

    Ok(HttpResponse::Ok().json(payouts))
}
