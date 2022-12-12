use tokio::sync::mpsc::{self, Receiver, Sender};

pub struct OrbitData {
    pub sender: Sender<OrbitEvent>,
    pub receiver: Receiver<OrbitEvent>,
}

impl OrbitData {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<OrbitEvent>(10_000);
        Self { sender, receiver }
    }
}

#[derive(Clone, Debug)]
pub struct OrbitEvent {
    pub exchange: String, // enum?
    pub symbol: String,
    pub contract_type: String,      //enum
    pub payload: OrbitEventPayload, //option
}

impl OrbitEvent {
    pub fn new(
        exchange: String,
        symbol: String,
        contract_type: String,
        payload: OrbitEventPayload,
    ) -> Self {
        Self {
            exchange,
            symbol,
            contract_type,
            payload,
        }
    }
}

#[derive(Clone, Debug)]
pub enum OrbitEventPayload {
    // OrderbookSnapshot(OrderbookUpdate),
    OrderbookUpdate(OrderbookUpdate),
}

// orderbook snapshots are just orderbooks updates with
// more levels and all them are "New" type
#[derive(Clone, Debug)]
pub struct OrderbookUpdate {
    pub timestamp: u64,
    pub bids: Vec<OrderbookUpdateLevel>,
    pub asks: Vec<OrderbookUpdateLevel>,
}

#[derive(Clone, Debug)]
pub struct OrderbookUpdateLevel(pub OrderbookUpdateType, pub f64, pub f64);

#[derive(Clone, Debug)]
pub enum OrderbookUpdateType {
    New,
    Change,
    Delete,
}
