use serde::Deserialize;


#[derive(Deserialize, Debug)]
pub struct DeribitInstrumentsWrapper {
    pub id: u64,
    pub jsonrpc: String,
    pub result: Vec<GetInstrumentsResponse>
}

#[derive(Deserialize, Debug)]
pub struct GetInstrumentsResponse {
    pub base_currency: String,
    pub block_trade_commission: f64,
    pub contract_size: u64,
    pub counter_currency: String,
    pub creation_timestamp: u64,
    pub expiration_timestamp: u64,
    pub future_type: String, 
    pub instrument_id: u64,
    pub instrument_name: String,
    pub is_active: bool,
    pub kind: String,
    pub leverage: u64,
    pub maker_commission: f64,
    pub min_trade_amount: f64,
    pub option_type: String,
    pub price_index: String, 
    pub quote_currency: String,
    pub settlement_period: String,
    pub strike: f64,
    pub taker_commision: f64,
    pub tick_size: f64,
}