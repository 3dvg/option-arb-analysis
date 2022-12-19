use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    OrbitContractType, OrbitEvent, OrbitEventPayload, OrbitExchange, OrbitInstrument,
    OrderbookUpdate, OrderbookUpdateLevel, OrderbookUpdateType,
};
use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use futures::{SinkExt, StreamExt};
use log::*;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug)]
pub struct DeribitClient {
    id: Uuid,
    sender: Sender<OrbitEvent>,
    receiver: Receiver<OrbitEvent>,
}

impl Default for DeribitClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DeribitClient {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel::<OrbitEvent>(10_000);
        Self {
            id: Uuid::new_v4(),
            sender,
            receiver,
        }
    }

    pub async fn get_currencies(&self) -> Result<DeribitCurrencyWrapper, Error> {
        let url = "https://test.deribit.com/api/v2/public/get_currencies";
        let response = reqwest::get(url).await?;
        let resp_text = response.text().await?;
        let resp_json = serde_json::from_str::<DeribitCurrencyWrapper>(&resp_text)
            .expect("Error parsing Deribit");
        Ok(resp_json)
    }
    // returns a vector because have to send a request per currency
    pub async fn get_instruments(&self) -> Result<Vec<DeribitInstrumentsWrapper>, Error> {
        let currencies = self.get_currencies().await?;
        let mut result = Vec::with_capacity(currencies.result.capacity());
        for c in currencies.result.iter() {
            let url = format!(
                "https://deribit.com/api/v2/public/get_instruments?currency={}&expired=false",
                c.currency
            );
            let response = reqwest::get(url).await?;
            let resp_text = response.text().await?;
            let resp_json = serde_json::from_str::<DeribitInstrumentsWrapper>(&resp_text)
                .expect("Error parsing Deribit");
            result.push(resp_json);
        }
        Ok(result)
    }

    pub async fn consume(
        &self,
        sender: Sender<OrbitEvent>,
        orbit_instruments: Vec<OrbitInstrument>,
    ) -> Result<(), Error> {
        tokio::spawn(Self::_stream_websocket_deribit(sender, orbit_instruments));
        Ok(())
    }

    pub async fn _stream_websocket_deribit(
        sender: Sender<OrbitEvent>,
        orbit_instruments: Vec<OrbitInstrument>,
    ) {
        let mut deribit_symbols = vec![];
        let mut symbol_details_map: HashMap<String, OrbitInstrument> = HashMap::new();
        for x in orbit_instruments.iter() {
            if x.contract_type == OrbitContractType::Future
                || x.contract_type == OrbitContractType::PutOption
                || x.contract_type == OrbitContractType::CallOption
                || x.contract_type == OrbitContractType::PerpetualFuture
                || x.contract_type == OrbitContractType::Spot
            {
                deribit_symbols.push(format!("book.{}.100ms", x.symbol)); //raw or 100ms
                symbol_details_map.insert(x.symbol.clone(), x.clone());
            }
        }

        let mut sleep = 100; //ms
        loop {
            // debug!("{:#?}",deribit_symbols);
            debug!("consuming deribit");
            let (mut stream, _response) = connect_async("wss://www.deribit.com/ws/api/v2")
                .await
                .expect("Expected connection with Deribit to work");

            debug!("initialized deribit stream");
            let result = stream
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "public/subscribe",
                        "id": 42,
                        "params": {
                        "channels": deribit_symbols}
                    })
                    .to_string(),
                ))
                .await;

            debug!("sent sub message to deribit (required): {:?}", result);

            let result = stream
                .send(Message::Text(
                    json!({
                        "jsonrpc" : "2.0",
                        "id" : 1003,
                        "method" : "public/set_heartbeat",
                        "params" : {
                        "interval" : 30
                        }
                    })
                    .to_string(),
                ))
                .await;
            debug!(
                "sent heartbeat sub message to deribit (recommended): {:?}",
                result
            );

            let mut hearbeat_timer: Instant = Instant::now();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            let resp = serde_json::from_str::<HashMap<String, Value>>(&text)
                                .expect("Error parsing Deribit");
                            match resp.get("method") {
                                Some(method) => {
                                    if let Value::String(method) = method {
                                        match method.as_str() {
                                            "subscription" => {
                                                let ob: DeribitOrderbookDataWrapper =
                                                    serde_json::from_str(&text)
                                                        .expect("Can't parse");
                                                        
                                                let norm_ob: OrbitEventPayload =
                                                    OrbitEventPayload::OrderbookUpdate(
                                                        OrderbookUpdate::from(
                                                            ob.params.data.clone(),
                                                        ),
                                                    );

                                                let contract_type = symbol_details_map
                                                    .get(&ob.params.data.instrument_name).map(|x| x.contract_type.clone());

                                                let currency = symbol_details_map
                                                    .get(&ob.params.data.instrument_name).map(|x| x.base.clone());

                                                let expiration = symbol_details_map
                                                    .get(&ob.params.data.instrument_name).and_then(|x| x.expiration_datetime);
                                                
                                                
                                                let orbit_event = OrbitEvent::new(
                                                    OrbitExchange::Deribit,
                                                    ob.params.data.instrument_name,
                                                    currency,
                                                    contract_type,
                                                    expiration,
                                                    Some(norm_ob),
                                                );
                                                // debug!("-- {:?}", ob);
                                                let _ = sender
                                                    .send(orbit_event)
                                                    .map_err(|err| error!("Error: {}", err));
                                            }
                                            "heartbeat" => {
                                                if hearbeat_timer.elapsed().as_secs() > 35 {
                                                    warn!("connection died, reconnecting...");
                                                    break;
                                                }
                                                hearbeat_timer = Instant::now();
                                                debug!("received heatbeat pong {:?}", text);
                                                let result = stream
                                                    .send(Message::Text(
                                                        json!({
                                                            "jsonrpc" : "2.0",
                                                            "id" : 8212,
                                                            "method" : "public/test",
                                                            "params" : {}
                                                        })
                                                        .to_string(),
                                                    ))
                                                    .await;
                                                debug!("sent heatbeat ping");
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                    Err(error) => {
                        error!("Error: {}", error);
                        break;
                    }
                }
            }
            // Exponential backoff
            warn!("Delta stream disconnected, re-connecting. Sleep:{}", sleep);
            tokio::time::sleep(Duration::from_millis(sleep)).await;
            sleep *= 2;
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct DeribitCurrencyWrapper {
    result: Vec<DeribitCurrency>,
}

#[derive(Deserialize, Debug)]
pub struct DeribitCurrency {
    currency: String,
}

#[derive(Clone, Debug)]
pub enum DeribitMarketEvent {
    OrderbookSnapshot(DeribitOrderbook),
}

#[derive(Deserialize, Debug)]
pub struct DeribitInstrumentsWrapper {
    pub id: Option<u64>,
    // pub jsonrpc: String,
    pub result: Vec<DeribitInstrument>,
}

#[derive(Deserialize, Debug)]
pub struct DeribitInstrument {
    pub base_currency: String,
    // pub block_trade_commission: f64,
    // pub contract_size: u64,
    pub counter_currency: String,
    pub creation_timestamp: u64,
    pub expiration_timestamp: i64, //deribit for perps uses expiration year 3000
    pub future_type: Option<String>,
    pub instrument_id: u64,
    pub instrument_name: String,
    pub is_active: bool,
    pub kind: DeribitInstrumentKind,
    // pub leverage: u64,
    // pub maker_commission: f64,
    // pub min_trade_amount: f64,
    pub option_type: Option<DeribitOptionType>,
    pub price_index: String,
    pub quote_currency: String,
    pub settlement_period: DeribitSettlementPeriod,
    pub strike: Option<f64>,
    // pub taker_commision: f64,
    // pub tick_size: f64,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeribitSettlementPeriod {
    Day,
    Week,
    Month,
    Perpetual,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeribitInstrumentKind {
    Future,
    Option,
    FutureCombo,
    OptionCombo,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DeribitOptionType {
    Put,
    Call,
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
    pub data: DeribitOrderbook,
}
#[derive(Deserialize, Debug, Clone)]
pub struct DeribitOrderbook {
    pub asks: Vec<DeribitOrderbookUpdate>,
    pub bids: Vec<DeribitOrderbookUpdate>,
    pub change_id: i64,
    pub instrument_name: String,
    pub prev_change_id: Option<i64>,
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub kind: DeribitOrderbookUpdateType,
}

impl From<&DeribitOrderbookUpdate> for OrderbookUpdateLevel {
    fn from(deribit_orderbook_level: &DeribitOrderbookUpdate) -> Self {
        let normalized_orderbook_update_type = match deribit_orderbook_level.0 {
            DeribitOrderbookAction::Change => OrderbookUpdateType::Change,
            DeribitOrderbookAction::Delete => OrderbookUpdateType::Delete,
            DeribitOrderbookAction::New => OrderbookUpdateType::New,
        };
        Self(
            normalized_orderbook_update_type,
            deribit_orderbook_level.1,
            deribit_orderbook_level.2,
        )
    }
}
impl From<DeribitOrderbook> for OrderbookUpdate {
    fn from(deribit_orderbook: DeribitOrderbook) -> Self {
        Self {
            timestamp: deribit_orderbook.timestamp,
            bids: deribit_orderbook
                .bids
                .iter()
                .map(OrderbookUpdateLevel::from)
                .collect(),
            asks: deribit_orderbook
                .asks
                .iter()
                .map(OrderbookUpdateLevel::from)
                .collect(),
        }
    }
}

impl From<&DeribitInstrument> for OrbitContractType {
    fn from(deribit_product: &DeribitInstrument) -> Self {
        match deribit_product.kind {
            DeribitInstrumentKind::Future => match deribit_product.settlement_period {
                DeribitSettlementPeriod::Perpetual => OrbitContractType::PerpetualFuture,
                _ => OrbitContractType::Future,
            },
            DeribitInstrumentKind::Option => match deribit_product.option_type.clone() {
                Some(option_type) => match option_type {
                    DeribitOptionType::Call => OrbitContractType::CallOption,
                    DeribitOptionType::Put => OrbitContractType::PutOption,
                },
                None => OrbitContractType::Unimplemented,
            },
            _ => OrbitContractType::Unimplemented,
        }
    }
}
impl From<&DeribitInstrument> for OrbitInstrument {
    fn from(deribit_product: &DeribitInstrument) -> Self {
        let contract_type = OrbitContractType::from(deribit_product);
        let strike = deribit_product.strike.map(|strike| strike as u64);

        let expiration_date = {
            let d: DateTime<Utc> = DateTime::from_utc(
                NaiveDateTime::from_timestamp_millis(deribit_product.expiration_timestamp).unwrap(),
                Utc,
            );
            let nd = d.date_naive();
            let t = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
            let a = NaiveDateTime::new(nd, t);
            let d: DateTime<Utc> = DateTime::from_local(a, Utc);
            let d = d.timestamp_millis();
            Some(d)
        };

        // debug!("deribit timestamp transformed {:?}, instrument {:?}", deribit_product.expiration_timestamp, deribit_product.instrument_name);
        Self {
            symbol: deribit_product.instrument_name.clone(),
            base: deribit_product.base_currency.clone(),
            quote: deribit_product.quote_currency.clone(),
            strike,
            expiration_datetime: Some(deribit_product.expiration_timestamp),
            expiration_date,
            contract_type,
            exchange: OrbitExchange::Deribit,
            //todo add native instrument name for websocket subs
        }
    }
}
