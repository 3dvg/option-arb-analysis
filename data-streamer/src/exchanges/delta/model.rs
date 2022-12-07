use serde::Deserialize;


pub enum DeltaSubscriptionType {
    Orderbook,
    Subscription,
    Heartbeat,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProductWrapper {
    // pub meta: DeltaProductWrapperMeta,
    pub success: bool,
    pub result: Vec<DeltaProduct>,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProduct {
    pub id: u64,
    pub symbol: String,
    #[serde(rename = "strike_price")]
    pub strike: Option<String>,
    pub contract_type: String,
    pub settlement_time: Option<String>,
    pub launch_time: Option<String>,
    pub underlying_asset: DeltaProductUnderlyingAsset,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProductUnderlyingAsset {
    pub symbol: String,
}

#[derive(Deserialize, Debug)]
pub struct DeltaHeartbeat {
    pub ts_origin: u64,
    pub ts_publish: u64,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize, Debug)]
pub struct DeltaOrderbook {
    pub buy: Vec<DeltaOrderbookLevel>,
    pub last_sequence_no: u64,
    pub last_updated_at: u64,
    pub product_id: u64,
    pub sell: Vec<DeltaOrderbookLevel>,
    pub symbol: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp: u64,
}

#[derive(Deserialize, Debug)]
pub struct DeltaOrderbookLevel {
    pub depth: String,
    pub limit_price: String,
    pub size: u64,
}
