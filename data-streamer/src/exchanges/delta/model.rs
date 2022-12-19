use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    OrbitContractType, OrbitEvent, OrbitEventPayload, OrbitExchange, OrbitInstrument,
    OrderbookUpdate, OrderbookUpdateLevel, OrderbookUpdateType, OrbitCurrency,
};
use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use futures::{SinkExt, StreamExt};
use log::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug)]
pub struct DeltaClient {
    id: Uuid,
    heartbeat_timeout: u64,
    sender: Sender<OrbitEvent>,
    receiver: Receiver<OrbitEvent>,
}

impl Default for DeltaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaClient {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel::<OrbitEvent>(10_000);
        Self {
            id: Uuid::new_v4(),
            heartbeat_timeout: 35,
            sender,
            receiver,
        }
    }

    pub async fn get_products(&self) -> Result<DeltaProductWrapper, Error> {
        let url = "https://api.delta.exchange/v2/products"; //TODO!
        let response = reqwest::get(url).await?;
        let resp_text = response.text().await?;
        let resp_json =
            serde_json::from_str::<DeltaProductWrapper>(&resp_text).expect("Error parsing Delta");
        Ok(resp_json)
    }

    pub async fn consume(
        &self,
        sender: Sender<OrbitEvent>,
        products: Vec<OrbitInstrument>,
    ) -> Result<(), Error> {
        // subscription forbidden on this channel with more than 20 symbols\",\"name\":\"l2_orderbook\"
        for chunk in products.chunks(20) {
            // let symbols: Vec<String> = chunk.iter().map(|p| p.symbol.clone()).collect();
            tokio::spawn(Self::_stream_websockets_delta(
                sender.clone(),
                chunk.to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn _stream_websockets_delta(
        sender: Sender<OrbitEvent>,
        symbols: Vec<OrbitInstrument>,
    ) {
        let mut symbol_details_map: HashMap<String, OrbitInstrument> = HashMap::new();
        let mut delta_symbols = vec![];
        for x in symbols.iter() {
            symbol_details_map.insert(x.symbol.clone(), x.clone());
            delta_symbols.push(x.symbol.clone());
        }
        let mut sleep = 100; //ms
        loop {
            let (mut stream, _response) = connect_async("wss://socket.delta.exchange")
                .await
                .expect("Expected connection with Delta to work");
            // debug!("initialized delta stream");
            let _result = stream
                .send(Message::Text(
                    json!({
                        "type": "subscribe",
                        "payload": {
                            "channels": [
                                {
                                    "name": "l2_orderbook",
                                    "symbols": delta_symbols
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .await;

            // debug!("sent sub message to delta (required): {:?}", result);

            let _result = stream
                .send(Message::Text(
                    json!({
                        "type": "enable_heartbeat"
                    })
                    .to_string(),
                ))
                .await;
            // debug!(
            //     "sent heartbeat sub message to delta (recommended): {:?}",
            //     result
            // );
            //TODO what happens when the channel gets clogged with messages??!!!!
            // right now, no logic and after 100k messages, the broadcast channel
            // length max was 365 messages
            let mut hearbeat_timer: Instant = Instant::now();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(msg) => {
                        match msg {
                            Message::Text(text) => {
                                let resp = serde_json::from_str::<HashMap<String, Value>>(&text)
                                    .expect("Error parsing Delta");
                                match resp.get("type").expect("Coundt find type in response") {
                                    Value::String(kind) => match kind.as_str() {
                                        "l2_orderbook" => {
                                            let ob: DeltaOrderbook =
                                                serde_json::from_str(&text).expect("Can't parse");
                                            // debug!("sent {:?}", ob);
                                            // if sender.send(DeltaMarketEvent::OrderbookSnapshot(ob)).is_err() {
                                            //     error!("error sending event");
                                            // };
                                            let norm_ob = OrbitEventPayload::OrderbookUpdate(
                                                OrderbookUpdate::from(ob.clone()),
                                            );

                                            let orbit_event: OrbitEvent = OrbitEvent {
                                                exchange: OrbitExchange::Delta,
                                                symbol: ob.symbol.clone(),
                                                currency: symbol_details_map.get(&ob.symbol).map(|x| x.base.clone()),
                                                contract_type: symbol_details_map.get(&ob.symbol).map(|x| x.contract_type.clone()),
                                                payload: Some(norm_ob),
                                                expiration: symbol_details_map.get(&ob.symbol).and_then(|x| x.expiration_date),
                                            };

                                            let _ = sender
                                                .send(orbit_event)
                                                .map_err(|err| error!("Error: {}", err));
                                        }
                                        "subscriptions" => {
                                            // let ds: DeltaSubscription =
                                            //     serde_json::from_str(&text).expect("Can't parse");
                                            // let _ = sender
                                            //     .send(DeltaMarketEvent::Subscription(ds))
                                            //     .map_err(|err| error!("Error: {}", err));
                                        }
                                        "heartbeat" => {
                                            if hearbeat_timer.elapsed().as_secs() > 35 {
                                                warn!("connection died, reconnecting...");
                                                break;
                                            }
                                            hearbeat_timer = Instant::now();
                                            // let hb: DeltaHeartbeat =
                                            //     serde_json::from_str(&text).expect("Can't parse");
                                            // let _ = sender.send(DeltaMarketEvent::Heartbeat(hb));
                                            // let latency = data.ts_publish - data.ts_origin;
                                            // debug!("heartbeat latency (ns) {:?}", latency);
                                        }
                                        _ => {
                                            error!("unexpected message");
                                            break;
                                        }
                                    },
                                    _ => {}
                                }
                            }
                            _ => {}
                        };
                        sleep = 100;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeltaMarketEvent {
    OrderbookSnapshot(DeltaOrderbook),
    Heartbeat(DeltaHeartbeat),
    Subscription(DeltaSubscription),
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
    pub contract_type: DeltaContractType,
    pub settlement_time: Option<String>,
    pub launch_time: Option<String>,
    pub underlying_asset: DeltaProductUnderlyingAsset,
    pub quoting_asset: DeltaProductQuotingAsset,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaContractType {
    Futures,
    PerpetualFutures,
    CallOptions,
    PutOptions,
    MoveOptions,
    Spot,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProductUnderlyingAsset {
    pub symbol: String,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProductQuotingAsset {
    pub symbol: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaHeartbeat {
    pub ts_origin: u64,
    pub ts_publish: u64,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaOrderbook {
    pub buy: Vec<DeltaOrderbookLevel>,
    // pub last_sequence_no: u64,
    // pub last_updated_at: u64,
    // pub product_id: u64,
    pub sell: Vec<DeltaOrderbookLevel>,
    pub symbol: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaOrderbookLevel {
    pub depth: String,
    pub limit_price: String,
    pub size: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaSubscription {
    pub channels: Vec<DeltaSubscriptionChannel>,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaSubscriptionChannel {
    pub name: String,
    pub symbols: Vec<String>,
}

impl From<&DeltaOrderbookLevel> for OrderbookUpdateLevel {
    fn from(delta_orderbook_level: &DeltaOrderbookLevel) -> Self {
        // orderbookupdatetype is new becuause delta does snapshots so OB is always new
        Self(
            OrderbookUpdateType::New,
            delta_orderbook_level.limit_price.parse::<f64>().unwrap(),
            delta_orderbook_level.size as f64,
        )
    }
}

impl From<DeltaOrderbook> for OrderbookUpdate {
    fn from(delta_orderbook: DeltaOrderbook) -> Self {
        Self {
            timestamp: delta_orderbook.timestamp,
            bids: delta_orderbook
                .buy
                .iter()
                .map(OrderbookUpdateLevel::from)
                .collect(),
            asks: delta_orderbook
                .sell
                .iter()
                .map(OrderbookUpdateLevel::from)
                .collect(),
        }
    }
}

impl From<&DeltaProduct> for OrbitInstrument {
    fn from(delta_product: &DeltaProduct) -> Self {
        let expiration_datetime = delta_product.settlement_time.clone().map(|time| {
            DateTime::parse_from_rfc3339(time.as_str())
                .unwrap()
                .timestamp_millis()
        });

        let expiration_date = delta_product.settlement_time.clone().map(|time| {
            let nd = DateTime::parse_from_rfc3339(&time).unwrap().date_naive();
            let t = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
            let a = NaiveDateTime::new(nd, t);
            let d: DateTime<Utc> = DateTime::from_local(a, Utc);
            d.timestamp_millis()
        });

        let strike = delta_product
            .strike
            .clone()
            .map(|strike| strike.parse::<u64>().unwrap());

        let contract_type = match delta_product.contract_type {
            DeltaContractType::Futures => OrbitContractType::Future,
            DeltaContractType::PerpetualFutures => OrbitContractType::PerpetualFuture,
            DeltaContractType::CallOptions => OrbitContractType::CallOption,
            DeltaContractType::PutOptions => OrbitContractType::PutOption,
            DeltaContractType::MoveOptions => OrbitContractType::MoveOption,
            DeltaContractType::Spot => OrbitContractType::Spot,
        };

        // debug!("delta timestamp transformed {:?}, settlement_time {:?}", expiration, delta_product.settlement_time);
        Self {
            symbol: delta_product.symbol.clone(),
            base: delta_product.underlying_asset.symbol.clone(),
            quote: delta_product.quoting_asset.symbol.clone(),
            strike,
            expiration_datetime,
            expiration_date,
            contract_type,
            exchange: OrbitExchange::Delta,
        }
    }
}
