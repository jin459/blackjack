pub mod games;
pub mod health;
pub mod metrics;
pub mod players;

use actix_web::web;

use crate::ws;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health)
        .service(metrics::prometheus_metrics)
        .service(
            web::scope("/api")
                .service(players::create_player)
                .service(players::get_player)
                .service(games::create_game)
                .service(games::list_games)
                .service(games::get_game)
                .service(games::join_game)
                .service(games::open_betting)
                .service(games::place_bet)
                .service(games::deal)
                .service(games::hit)
                .service(games::stand)
                .service(games::play_dealer)
                .service(games::settle),
        )
        .service(
            web::scope("/ws")
                .route("/games/{id}", web::get().to(ws::ws_game)),
        );
}
