use redis::{aio::ConnectionManager, Client};
use std::sync::Arc;

use domain::repository::{EventPublisher, GameRepository, PlayerRepository};

use crate::{
    redis_cache::{RedisGameRepository, RedisPlayerRepository},
    redis_streams::RedisEventPublisher,
};

/// All repository/publisher instances wired together from a single Redis connection.
pub struct Repos {
    pub games: Arc<dyn GameRepository>,
    pub players: Arc<dyn PlayerRepository>,
    pub events: Arc<dyn EventPublisher>,
}

/// Open a Redis `ConnectionManager` (multiplexed, auto-reconnect).
pub async fn connect(url: &str) -> anyhow::Result<ConnectionManager> {
    let client = Client::open(url)?;
    let manager = ConnectionManager::new(client).await?;
    Ok(manager)
}

/// Build all repos sharing the same connection manager.
pub fn build_repos(conn: ConnectionManager) -> Repos {
    Repos {
        games: Arc::new(RedisGameRepository::new(conn.clone())),
        players: Arc::new(RedisPlayerRepository::new(conn.clone())),
        events: Arc::new(RedisEventPublisher::new(conn)),
    }
}
