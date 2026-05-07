mod app_state;
mod config;
mod error;
mod metrics;
mod routes;
mod telemetry;
mod ws;

use actix_web::{web, App, HttpServer};
use app_state::AppState;
use application::handlers::{GameCommandService, GameQueryService};
use std::sync::Arc;
use tracing_actix_web::TracingLogger;
use ws::Broadcaster;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cfg = config::AppConfig::from_env()?;

    // Metrics first — sets global MeterProvider before OTel tracer init
    let (app_metrics, prom_registry) = metrics::init_metrics()?;

    telemetry::init(&cfg.otel_service_name, &cfg.otel_exporter_otlp_endpoint)?;

    tracing::info!(
        addr = cfg.bind_addr(),
        redis = cfg.redis_url,
        "starting blackjack API"
    );

    let conn = infrastructure::repositories::connect(&cfg.redis_url).await?;
    let repos = infrastructure::repositories::build_repos(conn.clone());

    let cmd = Arc::new(GameCommandService::new(
        repos.games.clone(),
        repos.players.clone(),
        repos.events.clone(),
    ));
    let qry = Arc::new(GameQueryService::new(repos.games, repos.players));

    let broadcaster = Arc::new(Broadcaster::new(1024));

    // Background task: Redis Streams → in-process broadcast
    ws::stream_reader::spawn(conn, broadcaster.clone());

    let state = web::Data::new(AppState {
        cmd,
        qry,
        broadcaster,
        metrics: Arc::new(app_metrics),
    });

    // Prometheus registry shared with the /metrics endpoint
    let prom_data = web::Data::new(prom_registry);

    let bind_addr = cfg.bind_addr();

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(prom_data.clone())
            .wrap(TracingLogger::default())
            .configure(routes::configure)
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    telemetry::shutdown();
    Ok(())
}
