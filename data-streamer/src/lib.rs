mod exchanges;
mod model;

use std::collections::HashSet;
use std::slice::Chunks;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use crate::exchanges::delta::model::{DeltaHeartbeat, DeltaOrderbook, DeltaProductWrapper};
use crate::exchanges::deribit::model::{DeribitOrderbookData, DeribitOrderbookDataWrapper};
use anyhow::Error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use exchanges::delta::model::{DeltaProduct, DeltaClient};
use exchanges::delta::*;
use exchanges::deribit::model::DeribitInstrumentsWrapper;
use futures::{SinkExt, StreamExt};
use log::*;
use model::{Event, Exchange, OrbitData};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[async_trait]
pub trait ExchangeAPI {
    const URL: &'static str;
    async fn get_products<T>() -> Result<T, Error>
    where
        T: DeserializeOwned;
    async fn consume(&self);
}

pub async fn get_delta_products() -> Result<DeltaProductWrapper, Error> {
    // let url = "https://api.delta.exchange/v2/products"; //TODO!
    // let response = reqwest::get(url).await?;
    // let resp_text = response.text().await?;
    // let resp_json =
    //     serde_json::from_str::<DeltaProductWrapper>(&resp_text).expect("Error parsing Delta");
    // Ok(resp_json)
    let resp_json =DeltaClient::get_products().await?;
    Ok((resp_json))
}

pub async fn consume_delta(delta_products: DeltaProductWrapper) -> Result<(), Error> {
    // subscription forbidden on this channel with more than 20 symbols\",\"name\":\"l2_orderbook\"

    let mut tasks = vec![];
    for (i, chunk) in delta_products.result.chunks(20).enumerate() {
        let symbols: Vec<String> = chunk.into_iter().map(|p| p.symbol.clone()).collect();
        tasks.push(tokio::spawn(stream_websockets_delta(i, symbols)));
        debug!("streamed ws {} with {} symbols", i, chunk.len());
    }
    let _join_result = futures::future::join_all(tasks).await;

    Ok(())
}

pub async fn stream_websockets_delta(ws_id: usize, symbols: Vec<String>) {
    let mut sleep = 100; //ms
    loop {
        debug!("consuming delta...");
        let (mut stream, _response) = connect_async("wss://socket.delta.exchange")
            .await
            .expect("Expected connection with Delta to work");
        debug!("initialized delta stream");
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

        debug!("sent sub message to delta (required): {:?}", result);

        let result = stream
            .send(Message::Text(
                json!({
                    "type": "enable_heartbeat"
                })
                .to_string(),
            ))
            .await;
        debug!(
            "sent heartbeat sub message to delta (recommended): {:?}",
            result
        );

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
                                        let data: DeltaOrderbook =
                                            serde_json::from_str(&text).expect("Can't parse");
                                        // debug!("{} {:?}", ws_id, data);
                                        debug!("streaming ws {}", ws_id);
                                    }
                                    "subscriptions" => {
                                        debug!("{:?}", text);
                                    }
                                    "heartbeat" => {
                                        if hearbeat_timer.elapsed().as_secs() > 35 {
                                            warn!("connection died, reconnecting...");
                                            break;
                                        }
                                        hearbeat_timer = Instant::now();
                                        let data: DeltaHeartbeat =
                                            serde_json::from_str(&text).expect("Can't parse");
                                        let latency = data.ts_publish - data.ts_origin;
                                        debug!("heartbeat latency (ns) {:?}", latency);
                                    }
                                    _ => warn!("unexpected"),
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

pub async fn get_deribit_products() -> Result<DeribitInstrumentsWrapper, Error> {
    let url = "https://deribit.com/api/v2/public/get_instruments?currency=BTC&expired=false"; //TODO!
    let response = reqwest::get(url).await?;
    let resp_text = response.text().await?;
    let resp_json = serde_json::from_str::<DeribitInstrumentsWrapper>(&resp_text)
        .expect("Error parsing Deribit");
    Ok(resp_json)
}

pub async fn consume_deribit(deribit_products: DeribitInstrumentsWrapper) {
    let mut sleep = 100; //ms
    loop {
        let mut deribit_symbols = vec![];
        for (_i, product) in deribit_products.result.iter().enumerate() {
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
                                        let data: DeribitOrderbookDataWrapper =
                                            serde_json::from_str(&text).expect("Can't parse");
                                        debug!("-- {:?}", data);
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
