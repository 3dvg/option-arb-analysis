use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Error;
use futures::{SinkExt, StreamExt};
use log::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

pub struct DeltaClient {
    id: Uuid,
    heartbeat_timeout: u64,
    sender: Sender<DeltaMarketEvent>,
    receiver: Receiver<DeltaMarketEvent>,
}

impl DeltaClient {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel::<DeltaMarketEvent>(10_000);
        Self {
            id: Uuid::new_v4(),
            heartbeat_timeout: 35,
            sender,
            receiver,
        }
    }

    pub fn get_receiver(&self) -> Receiver<DeltaMarketEvent> {
        self.sender.subscribe()
    }

    pub async fn get_delta_products(&self) -> Result<DeltaProductWrapper, Error> {
        let url = "https://api.delta.exchange/v2/products"; //TODO!
        let response = reqwest::get(url).await?;
        let resp_text = response.text().await?;
        let resp_json =
            serde_json::from_str::<DeltaProductWrapper>(&resp_text).expect("Error parsing Delta");
        Ok(resp_json)
    }

    pub async fn consume_delta(
        &self,
        delta_products: DeltaProductWrapper,
    ) -> Result<Receiver<DeltaMarketEvent>, Error> {
        // subscription forbidden on this channel with more than 20 symbols\",\"name\":\"l2_orderbook\"

        // let mut tasks = vec![];
        for (i, chunk) in delta_products.result.chunks(20).enumerate() {
            let symbols: Vec<String> = chunk.into_iter().map(|p| p.symbol.clone()).collect();
            // tasks.push(tokio::spawn(Self::_stream_websockets_delta(self.sender.clone(), symbols)));
            tokio::spawn(Self::_stream_websockets_delta(self.sender.clone(), symbols));
            // debug!("streamed ws {} with {} symbols", i, chunk.len());
        }
        // let _join_result = futures::future::join_all(tasks).await;
        Ok(self.sender.subscribe())
    }

    pub async fn _stream_websockets_delta(sender: Sender<DeltaMarketEvent>, symbols: Vec<String>) {
        let mut sleep = 100; //ms
        loop {
            // sender = sender.clone();
            debug!("consuming delta...");
            let (mut stream, _response) = connect_async("wss://socket.delta.exchange")
                .await
                .expect("Expected connection with Delta to work");
            // debug!("initialized delta stream");
            let result = stream
                .send(Message::Text(
                    json!({
                        "type": "subscribe",
                        "payload": {
                            "channels": [
                                {
                                    "name": "l2_orderbook",
                                    "symbols": symbols
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .await;

            // debug!("sent sub message to delta (required): {:?}", result);

            let result = stream
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
                                            let _ = sender
                                                .send(DeltaMarketEvent::OrderbookSnapshot(ob))
                                                .map_err(|err| error!("Error: {}", err));
                                        }
                                        "subscriptions" => {
                                            let ds: DeltaSubscription =
                                                serde_json::from_str(&text).expect("Can't parse");
                                            let _ = sender
                                                .send(DeltaMarketEvent::Subscription(ds))
                                                .map_err(|err| error!("Error: {}", err));
                                        }
                                        "heartbeat" => {
                                            if hearbeat_timer.elapsed().as_secs() > 35 {
                                                warn!("connection died, reconnecting...");
                                                break;
                                            }
                                            hearbeat_timer = Instant::now();
                                            let hb: DeltaHeartbeat =
                                                serde_json::from_str(&text).expect("Can't parse");
                                            let _ = sender.send(DeltaMarketEvent::Heartbeat(hb));
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
    pub contract_type: String,
    pub settlement_time: Option<String>,
    pub launch_time: Option<String>,
    pub underlying_asset: DeltaProductUnderlyingAsset,
}

#[derive(Deserialize, Debug)]
pub struct DeltaProductUnderlyingAsset {
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
    // pub timestamp: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeltaOrderbookLevel {
    pub depth: String,
    // pub limit_price: String,
    // pub size: u64,
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
