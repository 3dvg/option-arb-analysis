use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct DeribitInstrumentsWrapper {
    pub id: Option<u64>,
    // pub jsonrpc: String,
    pub result: Vec<DeribitInstruments>,
}

#[derive(Deserialize, Debug)]
pub struct DeribitInstruments {
    pub base_currency: String,
    // pub block_trade_commission: f64,
    // pub contract_size: u64,
    pub counter_currency: String,
    pub creation_timestamp: u64,
    pub expiration_timestamp: u64,
    pub future_type: Option<String>,
    pub instrument_id: u64,
    pub instrument_name: String,
    pub is_active: bool,
    pub kind: String,
    // pub leverage: u64,
    // pub maker_commission: f64,
    // pub min_trade_amount: f64,
    pub option_type: Option<String>,
    pub price_index: String,
    pub quote_currency: String,
    pub settlement_period: String,
    pub strike: Option<f64>,
    // pub taker_commision: f64,
    // pub tick_size: f64,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeribitOrderbookAction {
    New,
    Change,
    Delete,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeribitOrderbookUpdate(pub DeribitOrderbookAction, pub f64, pub f64);

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DeribitOrderbookUpdateType {
    Change,
    Snapshot,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeribitOrderbookDataWrapper {
    pub method: String,
    pub params: DeribitOrderbookDataParams,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeribitOrderbookDataParams {
    pub channel: String,
    pub data: DeribitOrderbookData,
}
#[derive(Deserialize, Debug, Clone)]
pub struct DeribitOrderbookData {
    pub asks: Vec<DeribitOrderbookUpdate>,
    pub bids: Vec<DeribitOrderbookUpdate>,
    pub change_id: i64,
    pub instrument_name: String,
    pub prev_change_id: Option<i64>,
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub kind: DeribitOrderbookUpdateType,
}
