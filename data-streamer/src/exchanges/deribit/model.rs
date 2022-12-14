use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    OrbitEvent, OrbitEventPayload, OrbitInstrument, OrderbookUpdate, OrderbookUpdateLevel,
    OrderbookUpdateType,
};
use anyhow::Error;
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

impl DeribitClient {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel::<OrbitEvent>(10_000);
        Self {
            id: Uuid::new_v4(),
            sender,
            receiver,
        }
    }

    pub async fn get_instruments(&self) -> Result<DeribitInstrumentsWrapper, Error> {
        let url = "https://deribit.com/api/v2/public/get_instruments?currency=BTC&expired=false"; //TODO!
        let response = reqwest::get(url).await?;
        let resp_text = response.text().await?;
        let resp_json = serde_json::from_str::<DeribitInstrumentsWrapper>(&resp_text)
            .expect("Error parsing Deribit");
        Ok(resp_json)
    }

    pub async fn consume(
        &self,
        deribit_products: DeribitInstrumentsWrapper,
    ) -> Result<Receiver<OrbitEvent>, Error> {
        tokio::spawn(Self::_stream_websocket_deribit(
            self.sender.clone(),
            deribit_products,
        ));
        Ok(self.sender.subscribe())
    }

    pub async fn _stream_websocket_deribit(
        sender: Sender<OrbitEvent>,
        deribit_products: DeribitInstrumentsWrapper,
    ) {
        let mut sleep = 100; //ms
        loop {
            let mut deribit_symbols = vec![];
            for product in deribit_products.result.iter() {
                if product.kind == "option" || product.kind == "future" {
                    deribit_symbols.push(format!("book.{}.100ms", product.instrument_name));
                    //raw or 100ms
                }
            }
            // debug!("{:#?}",deribit_symbols);
            debug!("consuming deribit...");
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
                    Ok(msg) => match msg {
                        Message::Text(text) => {
                            let resp = serde_json::from_str::<HashMap<String, Value>>(&text)
                                .expect("Error parsing Deribit");
                            match resp.get("method") {
                                Some(method) => match method {
                                    Value::String(method) => match method.as_str() {
                                        "subscription" => {
                                            let ob: DeribitOrderbookDataWrapper =
                                                serde_json::from_str(&text).expect("Can't parse");
                                            let norm_ob: OrbitEventPayload =
                                                OrbitEventPayload::OrderbookUpdate(
                                                    OrderbookUpdate::from(ob.params.data.clone()),
                                                );
                                            let orbit_event = OrbitEvent::new(
                                                "deribit".to_string(),
                                                ob.params.data.instrument_name,
                                                "futures or options".to_string(),
                                                norm_ob,
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
                                            debug!("sent heatbeat ping {:?}", result);
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                },
                                None => {}
                            }
                        }
                        _ => {}
                    },
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
        // Self(
        //     normalized_orderbook_update_type,
        //     deribit_orderbook_level.1,
        //     deribit_orderbook_level.2,
        // )
        todo!()
    }
}
impl From<DeribitOrderbook> for OrderbookUpdate {
    fn from(deribit_orderbook: DeribitOrderbook) -> Self {
        Self {
            timestamp: deribit_orderbook.timestamp,
            bids: deribit_orderbook
                .bids
                .iter()
                .map(|der_ob| OrderbookUpdateLevel::from(der_ob))
                .collect(),
            asks: deribit_orderbook
                .asks
                .iter()
                .map(|der_ob| OrderbookUpdateLevel::from(der_ob))
                .collect(),
        }
    }
}

impl From<&DeribitInstrument> for OrbitInstrument {
    fn from(deribit_product: &DeribitInstrument) -> Self {
        Self {
            symbol: deribit_product.instrument_name.clone(),
        }
    }
}
