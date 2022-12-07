mod exchanges;

use std::collections::HashSet;
use std::{collections::HashMap, time::Instant};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use log::*;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use exchanges::delta::*;
use log::*;
use anyhow::Error;
use crate::exchanges::delta::model::{DeltaOrderbook, DeltaHeartbeat, DeltaProductWrapper};

pub enum Exchange {
    Deribit,
    Delta,
}

pub async fn get_delta_products() -> Result<(), Error>{
    let url = "https://api.delta.exchange/v2/products?page_size=100"; //TODO!

    let response = reqwest::get(url).await?;
    let resp_text = response.text().await?;
    let resp_json = serde_json::from_str::<DeltaProductWrapper>(&resp_text).expect("Error parsing Delta");

    let mut delta_strikes: HashSet<u64> = HashSet::with_capacity(resp_json.result.capacity());
    let mut delta_underlying_assets: HashSet<String> = HashSet::with_capacity(resp_json.result.capacity());

    for product in resp_json.result {
        debug!("response {:#?}", product);
        let _ = match product.strike {
            Some(strike) => delta_strikes.insert(strike.parse::<u64>()?),
            None => false,
        };
        delta_underlying_assets.insert(product.underlying_asset.symbol);
    }

    debug!("delta_strikes {:?}", delta_strikes);
    debug!("delta_underlying_assets {:?}", delta_underlying_assets);
    Ok(())
}

pub async fn consume_delta() {
    let mut sleep = 100; //ms
    loop {
        debug!("consuming delta...");
        let (mut stream, response) = connect_async("wss://socket.delta.exchange").await.expect("Expected connection with Binance to work");
        debug!("initialized delta stream");
        let result = stream.send(Message::Text(
            json!({
                "type": "subscribe",
                "payload": {
                    "channels": [
                        {
                            "name": "l2_orderbook",
                            "symbols": [
                                "P-ETH-950-301222",
                            ]
                        }
                    ]
                }
            }).to_string(),
        )).await;

        debug!("sent sub message to delta (required): {:?}", result);

        let result = stream.send(Message::Text(
            json!({
                "type": "enable_heartbeat"
            }).to_string(),
        )).await;
        debug!("sent heartbeat sub message to delta (recommended): {:?}", result);

        let mut hearbeat_timer: Instant = Instant::now();
        while let Some(event) = stream.next().await {
            match event {
                Ok(msg) => {
                    match msg {
                        Message::Text(text) => {
                            let resp = serde_json::from_str::<HashMap<String, Value>>(&text).expect("Error parsing Delta");
                            match resp.get("type").expect("Coundt find type in response") {
                                Value::String(kind) => {
                                    match kind.as_str() {
                                        "l2_orderbook" => {
                                            let data: DeltaOrderbook = serde_json::from_str(&text).expect("Can't parse");
                                            debug!("{:?}", data);
                                        }
                                        "subscriptions" => {
                                            debug!("{:?}", text);
                                        }
                                        "heartbeat" => {
                                            if hearbeat_timer.elapsed().as_secs() > 35 {
                                                warn!("connection died, reconnecting...");
                                                break
                                            }
                                            hearbeat_timer  = Instant::now();
                                            let data: DeltaHeartbeat = serde_json::from_str(&text).expect("Can't parse");
                                            let latency = data.ts_publish - data.ts_origin;
                                            debug!("heartbeat latency (ns) {:?}", latency);
                                        }
                                        _ => warn!("unexpected")
                                    }
                                }
                                _ => {}
                            }
                        }
                        Message::Binary(bin) => {}
                        Message::Ping(ping) => {}
                        Message::Pong(pong) => {}
                        Message::Close(_) => {}
                        Message::Frame(_) => {}
                    };
                    sleep = 100;
                }
                Err(error) => {
                    error!("Error: {}", error);
                    break
                }
            }
        }        
        // Exponential backoff
        warn!("Delta stream disconnected, re-connecting. Sleep:{}", sleep);
        tokio::time::sleep(Duration::from_millis(sleep)).await;
        sleep *= 2;
    }
}
