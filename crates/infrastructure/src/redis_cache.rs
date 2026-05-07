use async_trait::async_trait;
use domain::{
    error::DomainError,
    game::{Game, GameStatus},
    ids::{GameId, PlayerId},
    player::Player,
    repository::{GameRepository, PlayerRepository},
};
use redis::{aio::ConnectionManager, AsyncCommands};
use tracing::instrument;

const GAME_TTL_SECS: u64 = 3600;
const ACTIVE_GAMES_KEY: &str = "bj:games:active";
const ALL_PLAYERS_KEY: &str = "bj:players:all";

fn game_key(id: GameId) -> String {
    format!("bj:game:{id}")
}

fn player_key(id: PlayerId) -> String {
    format!("bj:player:{id}")
}

fn repo_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}

// ---------------------------------------------------------------------------
// Game repository
// ---------------------------------------------------------------------------

pub struct RedisGameRepository {
    conn: ConnectionManager,
}

impl RedisGameRepository {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl GameRepository for RedisGameRepository {
    #[instrument(skip(self), fields(game.id = %game.id()))]
    async fn save(&self, game: &Game) -> Result<(), DomainError> {
        let mut c = self.conn.clone();
        let key = game_key(game.id());
        let json = serde_json::to_string(game).map_err(repo_err)?;

        let _: () = c.set_ex(&key, &json, GAME_TTL_SECS).await.map_err(repo_err)?;

        let id_str = game.id().to_string();
        if matches!(game.status(), GameStatus::Settled) {
            let _: i64 = c.srem(ACTIVE_GAMES_KEY, &id_str).await.map_err(repo_err)?;
        } else {
            let _: i64 = c.sadd(ACTIVE_GAMES_KEY, &id_str).await.map_err(repo_err)?;
        }

        tracing::debug!("game saved to redis");
        Ok(())
    }

    #[instrument(skip(self), fields(game.id = %id))]
    async fn find_by_id(&self, id: GameId) -> Result<Option<Game>, DomainError> {
        let mut c = self.conn.clone();
        let json: Option<String> = c.get(game_key(id)).await.map_err(repo_err)?;
        match json {
            None => Ok(None),
            Some(j) => Ok(Some(serde_json::from_str(&j).map_err(repo_err)?)),
        }
    }

    #[instrument(skip(self))]
    async fn list_active(&self) -> Result<Vec<Game>, DomainError> {
        let mut c = self.conn.clone();
        let ids: Vec<String> = c.smembers(ACTIVE_GAMES_KEY).await.map_err(repo_err)?;

        let mut games = Vec::with_capacity(ids.len());
        for id_str in ids {
            let key = format!("bj:game:{id_str}");
            let json: Option<String> = c.get(&key).await.map_err(repo_err)?;
            if let Some(j) = json {
                match serde_json::from_str::<Game>(&j) {
                    Ok(g) => games.push(g),
                    Err(e) => tracing::warn!(key, err = %e, "failed to deserialize game, skipping"),
                }
            }
        }
        Ok(games)
    }

    #[instrument(skip(self), fields(game.id = %id))]
    async fn delete(&self, id: GameId) -> Result<(), DomainError> {
        let mut c = self.conn.clone();
        let _: i64 = c.del(game_key(id)).await.map_err(repo_err)?;
        let _: i64 = c.srem(ACTIVE_GAMES_KEY, id.to_string()).await.map_err(repo_err)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Player repository
// ---------------------------------------------------------------------------

pub struct RedisPlayerRepository {
    conn: ConnectionManager,
}

impl RedisPlayerRepository {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl PlayerRepository for RedisPlayerRepository {
    #[instrument(skip(self), fields(player.id = %player.id()))]
    async fn save(&self, player: &Player) -> Result<(), DomainError> {
        let mut c = self.conn.clone();
        let key = player_key(player.id());
        let json = serde_json::to_string(player).map_err(repo_err)?;

        let _: () = c.set(&key, &json).await.map_err(repo_err)?;
        let _: i64 = c.sadd(ALL_PLAYERS_KEY, player.id().to_string()).await.map_err(repo_err)?;

        tracing::debug!("player saved to redis");
        Ok(())
    }

    #[instrument(skip(self), fields(player.id = %id))]
    async fn find_by_id(&self, id: PlayerId) -> Result<Option<Player>, DomainError> {
        let mut c = self.conn.clone();
        let json: Option<String> = c.get(player_key(id)).await.map_err(repo_err)?;
        match json {
            None => Ok(None),
            Some(j) => Ok(Some(serde_json::from_str(&j).map_err(repo_err)?)),
        }
    }

    #[instrument(skip(self))]
    async fn find_all(&self) -> Result<Vec<Player>, DomainError> {
        let mut c = self.conn.clone();
        let ids: Vec<String> = c.smembers(ALL_PLAYERS_KEY).await.map_err(repo_err)?;

        let mut players = Vec::with_capacity(ids.len());
        for id_str in ids {
            let key = format!("bj:player:{id_str}");
            let json: Option<String> = c.get(&key).await.map_err(repo_err)?;
            if let Some(j) = json {
                match serde_json::from_str::<Player>(&j) {
                    Ok(p) => players.push(p),
                    Err(e) => tracing::warn!(key, err = %e, "failed to deserialize player, skipping"),
                }
            }
        }
        Ok(players)
    }

    #[instrument(skip(self), fields(player.id = %id))]
    async fn delete(&self, id: PlayerId) -> Result<(), DomainError> {
        let mut c = self.conn.clone();
        let _: i64 = c.del(player_key(id)).await.map_err(repo_err)?;
        let _: i64 = c.srem(ALL_PLAYERS_KEY, id.to_string()).await.map_err(repo_err)?;
        Ok(())
    }
}
