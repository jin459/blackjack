use anyhow::Context;
use opentelemetry::metrics::{Counter, UpDownCounter};
use opentelemetry::global;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::Registry;

/// Domain-level metrics recorded at the API boundary.
/// All counters flow through OTel → Prometheus registry → `/metrics` endpoint.
pub struct AppMetrics {
    pub games_created: Counter<u64>,
    pub bets_placed: Counter<u64>,
    /// Total chips wagered across all bets.
    pub chips_wagered: Counter<u64>,
    pub cards_dealt: Counter<u64>,
    pub active_games: UpDownCounter<i64>,
    pub ws_connections: UpDownCounter<i64>,
}

impl AppMetrics {
    fn new() -> Self {
        let meter = global::meter("blackjack");
        Self {
            games_created: meter
                .u64_counter("bj.games.created")
                .with_description("Total game tables created")
                .init(),
            bets_placed: meter
                .u64_counter("bj.bets.placed")
                .with_description("Total bets placed")
                .init(),
            chips_wagered: meter
                .u64_counter("bj.chips.wagered")
                .with_description("Total chips wagered")
                .init(),
            cards_dealt: meter
                .u64_counter("bj.cards.dealt")
                .with_description("Total cards dealt")
                .init(),
            active_games: meter
                .i64_up_down_counter("bj.games.active")
                .with_description("Currently active game tables")
                .init(),
            ws_connections: meter
                .i64_up_down_counter("bj.ws.connections")
                .with_description("Active WebSocket connections")
                .init(),
        }
    }
}

/// Initialise the Prometheus exporter as the global OTel MeterProvider.
/// Returns `(AppMetrics, prometheus::Registry)`.
pub fn init_metrics() -> anyhow::Result<(AppMetrics, Registry)> {
    let registry = Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()
        .context("failed to build Prometheus exporter")?;

    let provider = SdkMeterProvider::builder()
        .with_reader(exporter)
        .build();

    global::set_meter_provider(provider);

    Ok((AppMetrics::new(), registry))
}
