use std::sync::Arc;
use tokio::sync::broadcast;

/// One message slot on the in-process broadcast channel.
#[derive(Debug, Clone)]
pub struct BroadcastMsg {
    /// Game this event belongs to (pre-extracted so WS sessions can filter cheaply).
    pub game_id: String,
    /// The full DomainEvent serialised as JSON — sent verbatim to the browser.
    pub payload: Arc<str>,
}

/// Shared publisher end of the broadcast channel.
/// Cheap to clone — holds only an `Arc` internally.
#[derive(Clone)]
pub struct Broadcaster {
    tx: broadcast::Sender<BroadcastMsg>,
}

impl Broadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe a new WebSocket session.
    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastMsg> {
        self.tx.subscribe()
    }

    /// Publish an event; silently drops if no active subscribers.
    pub fn publish(&self, msg: BroadcastMsg) {
        let _ = self.tx.send(msg);
    }
}
