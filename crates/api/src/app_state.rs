use application::handlers::{GameCommandService, GameQueryService};
use std::sync::Arc;

use crate::metrics::AppMetrics;
use crate::ws::Broadcaster;

pub struct AppState {
    pub cmd: Arc<GameCommandService>,
    pub qry: Arc<GameQueryService>,
    pub broadcaster: Arc<Broadcaster>,
    pub metrics: Arc<AppMetrics>,
}
