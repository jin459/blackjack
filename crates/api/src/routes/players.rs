use actix_web::{get, post, web, HttpResponse};
use application::{
    commands::CreatePlayerCommand,
    queries::GetPlayerQuery,
};
use domain::ids::PlayerId;
use serde::Deserialize;
use uuid::Uuid;

use crate::{app_state::AppState, error::ApiError};

#[derive(Deserialize)]
pub struct CreatePlayerBody {
    pub name: String,
    pub initial_balance: u32,
}

#[post("/players")]
pub async fn create_player(
    state: web::Data<AppState>,
    body: web::Json<CreatePlayerBody>,
) -> Result<HttpResponse, ApiError> {
    let dto = state
        .cmd
        .create_player(CreatePlayerCommand {
            name: body.name.clone(),
            initial_balance: body.initial_balance,
        })
        .await?;
    Ok(HttpResponse::Created().json(dto))
}

#[get("/players/{id}")]
pub async fn get_player(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let dto = state
        .qry
        .get_player(GetPlayerQuery {
            player_id: PlayerId::from_uuid(path.into_inner()),
        })
        .await?;
    Ok(HttpResponse::Ok().json(dto))
}
