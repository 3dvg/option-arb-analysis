use std::collections::{BTreeMap, HashMap};
// use std::hash::Hash;
// use std::sync::{Arc, Mutex};

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
// use std::{option, thread};

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use deribit::models::subscription::{Greeks, TickerData};
use deribit::models::{
    AssetKind, Currency, GetInstrumentsRequest, GetInstrumentsResponse, HeartbeatType,
    PublicSubscribeRequest, SetHeartbeatRequest, SubscriptionData, SubscriptionParams, TestRequest,
};
use deribit::{DeribitBuilder, DeribitError};
use dotenv::dotenv;
use env_logger::init;
use fehler::throws;
use futures::StreamExt;

use ordered_float::OrderedFloat;
use tokio::sync::RwLock;

const CONNECTION: &'static str = "wss://www.deribit.com/ws/api/v2";
#[derive(Debug)]
pub struct Instruments {
    // pub id: u8,
    pub instrument_name: String,
    pub kind: String,
    pub expiration_timestamp: i64,
    pub is_active: bool,
    // pub timestamp: DateTime<Utc>
}

#[derive(PartialEq, Clone, Debug)]
pub enum InstrumentType {
    BTC,
    ETH,
}

#[throws(Error)]
#[tokio::main]
async fn main() {
    let _ = dotenv();
    init();

    println!("Connecting to {}", CONNECTION);

    let drb = DeribitBuilder::default().testnet(false).build().unwrap();

    let (mut client, mut subscription) = drb.connect().await?;

    // the options that have futures expirations in common are easy to hedge
    // what about the ones that dont? they have synthetic future as guideline, we are interested as well... BUT,
    // how do we do it? perp? build our own syn future w/ term structure?
    let futures_instruments = client
        .call(GetInstrumentsRequest::futures(Currency::BTC))
        .await?
        .await?;

    let futures_expirations: Vec<u64> = futures_instruments
        .iter()
        .map(|f| f.expiration_timestamp)
        .collect();

    // println!("futures_instruments {:#?}", futures_expirations);

    let option_instruments = client
        .call(GetInstrumentsRequest::options(Currency::BTC))
        .await?
        .await?;

    let option_chains = OptionChains::new(&option_instruments);

    let mut channels = vec![];

    // let first_expiration = option_chains.get_first_n_expirations(5);
    //using futures expirations for now
    let first_expiration = futures_expirations[0];
    let mut first_chain = option_chains
        .get_option_chain(first_expiration)
        .await
        .unwrap();

    println!("first_chain {:?}", first_chain);

    for (_strike, contract) in first_chain.contracts.iter()
    {
        let inst_str = format!(
            "ticker.{}.100ms",
            contract.instrument_name.clone().unwrap()
        );
        channels.push(inst_str);
    }
    // for ((_call_strike, call_contract), (_put_strike, put_contract)) in
    //     first_chain.calls.iter().zip(first_chain.puts.iter())
    // {
    //     let inst_str = format!(
    //         "ticker.{}.100ms",
    //         call_contract.instrument_name.clone().unwrap()
    //     );
    //     channels.push(inst_str);
    //     let inst_str = format!(
    //         "ticker.{}.100ms",
    //         put_contract.instrument_name.clone().unwrap()
    //     );
    //     channels.push(inst_str);
    // }

    println!("channels {:#?}", channels);



    let req = PublicSubscribeRequest::new(&channels);
    let _ = client.call(req).await?.await?;

    while let Some(m) = subscription.next().await {
        match m?.params {
            SubscriptionParams::Heartbeat { r#type: ty } => match ty {
                HeartbeatType::TestRequest => {
                    println!("Test Requested");
                    client.call(TestRequest::default()).await?;
                }
                _ => println!("Heartbeat"),
            },
            SubscriptionParams::Subscription(subscription_data) => match subscription_data {
                SubscriptionData::Ticker(data_channel) => {
                    let data: TickerData = data_channel.data.clone();
                    println!("-- {:?}", data);
                    first_chain
                        .contracts
                        .entry(data.instrument_name.clone())
                        .and_modify(|contract| {
                            contract.ask_amount = Some(data.best_ask_amount);
                            contract.ask_price = data.best_ask_price;
                            contract.ask_iv = data.ask_iv;
                            contract.bid_amount = Some(data.best_bid_amount);
                            contract.bid_price = data.best_bid_price;
                            contract.bid_iv = data.bid_iv;
                            contract.mark_price = Some(data.mark_price);
                            contract.mark_iv = data.mark_iv;
                            contract.instrument_name = Some(data.instrument_name);
                            contract.underlying_index = data.underlying_index;
                            contract.underlying_price = data.underlying_price;
                            contract.greeks = data.greeks;
                            contract.timestamp = Some(data.timestamp);
                        });
                        println!("=={:?}", first_chain.contracts.get(&data_channel.data.instrument_name));
                }
                _ => {}
            },
        }
    }
}

fn filter_options_by_delta() {}

pub fn get_datetime(timestamp: u64) -> Result<DateTime<Utc>, Error> {
    let d = UNIX_EPOCH + Duration::from_millis(timestamp);
    let datetime = DateTime::<Utc>::from(d);
    // let timestamp_str = datetime.format("%Y-%m-%d %H:%M:%S.%f").to_string();
    Ok(datetime)
}

pub type Expiration = u64;
pub type Strike = OrderedFloat<f64>;
pub type StrikeDiff = OrderedFloat<f64>;

#[derive(Debug)]
pub struct OptionChains {
    pub chains: Arc<RwLock<BTreeMap<Expiration, OptionChain>>>,
}

impl OptionChains {
    pub fn new(instruments: &Vec<GetInstrumentsResponse>) -> Self {
        let mut chains: BTreeMap<u64, OptionChain> = BTreeMap::new();
        for instrument in instruments.iter() {
            chains.insert(instrument.expiration_timestamp, OptionChain::default());
        }

        for instrument in instruments.iter() {
            // println!("option type {:?}", instrument.option_type);
            let chain = chains.get_mut(&instrument.expiration_timestamp);
            // let strike = OrderedFloat(instrument.strike.unwrap());
            let instrument_name = instrument.instrument_name.clone();
            chain
                .unwrap()
                .contracts
                .insert(instrument_name, OptionContract::from(instrument.clone()));
            // let _ = match instrument.option_type.as_ref().unwrap().as_str() {
            //     "put" => chain
            //         .unwrap()
            //         .puts
            //         .insert(instrument_name, OptionContract::from(instrument.clone())),
            //     "call" => chain
            //         .unwrap()
            //         .calls
            //         .insert(instrument_name, OptionContract::from(instrument.clone())),
            //     _ => panic!("option type not implemented"),
            // };
        }
        Self {
            chains: Arc::new(RwLock::new(chains)),
        }
    }

    pub async fn get_option_chain(&self, expiration: u64) -> Option<OptionChain> {
        // self.chains.get(&expiration).cloned()
        let lock = self.chains.read().await;
        lock.get(&expiration).cloned()
    }

    pub async fn get_expirations(&self) -> Vec<u64> {
        // self.chains.keys().map(|e| *e).collect()
        let lock = self.chains.read().await;
        lock.keys().map(|e| *e).collect()
    }

    pub async fn get_expirations_datetime(&self) -> Vec<DateTime<Utc>> {
        // self.chains
        //     .keys()
        //     .map(|e| get_datetime(*e).unwrap())
        //     .collect()
        let lock = self.chains.read().await;
        lock.keys().map(|e| get_datetime(*e).unwrap()).collect()
    }

    pub async fn get_first_n_expirations(&self, n: usize) -> Vec<u64> {
        // self.chains.keys().map(|e| *e).take(n).collect()
        let lock = self.chains.read().await;
        lock.keys().map(|e| *e).take(n).collect()
    }

    pub async fn get_first_n_expirations_datetime(&self, n: usize) -> Vec<DateTime<Utc>> {
        let lock = self.chains.read().await;
        // self.chains
        //     .keys()
        //     .map(|e| get_datetime(*e).unwrap())
        //     .take(n)
        //     .collect()
        lock.keys()
            .map(|e| get_datetime(*e).unwrap())
            .take(n)
            .collect()
    }
    // pub fn insert_instrument(&mut self, instrument: &GetInstrumentsResponse) {
    // self.chains.insert(instrument.expiration_timestamp, );
    // }
}

#[derive(Debug, Default, Clone)]
pub struct OptionChain {
    // pub base: String,
    // pub quote: String,
    // pub symbol: String,
    // pub expiration: DateTime<Utc>,
    // pub settlement_period: String,
    // pub contract_size: f64,
    // pub min_trade_amount: f64,
    pub contracts: HashMap<String, OptionContract>,
    pub calls: HashMap<String, OptionContract>,
    pub puts: HashMap<String, OptionContract>,
    pub calls_sort: BTreeMap<StrikeDiff, OptionContract>,
    pub puts_sort: BTreeMap<StrikeDiff, OptionContract>,
    // pub atm_call: OptionContract,
    // pub atm_put: OptionContract,
    // pub is_active: bool,
}

impl OptionChain {
    pub fn insert() {}
}

#[derive(Debug, Clone)]
pub struct OptionContract {
    pub strike: Option<f64>,
    pub ask_amount: Option<f64>,
    pub ask_price: Option<f64>,
    pub ask_iv: Option<f64>,
    pub bid_amount: Option<f64>,
    pub bid_price: Option<f64>,
    pub bid_iv: Option<f64>,
    pub mark_price: Option<f64>,
    pub mark_iv: Option<f64>,
    pub option_type: Option<String>,
    pub instrument_name: Option<String>,
    pub underlying_index: Option<String>,
    pub underlying_price: Option<f64>,
    pub greeks: Option<Greeks>,
    pub expiration_timestamp: Option<u64>,
    pub timestamp: Option<u64>,
}

impl From<GetInstrumentsResponse> for OptionContract {
    fn from(instrument: GetInstrumentsResponse) -> OptionContract {
        Self {
            strike: Some(instrument.strike.unwrap()),
            ask_amount: None,
            ask_price: None,
            ask_iv: None,
            bid_amount: None,
            bid_price: None,
            bid_iv: None,
            mark_price: None,
            mark_iv: None,
            option_type: Some(instrument.option_type.unwrap()),
            instrument_name: Some(instrument.instrument_name),
            expiration_timestamp: Some(instrument.expiration_timestamp),
            underlying_index: None,
            underlying_price: None,
            greeks: None,
            timestamp: None,
        }
    }
}

// impl From<TickerData> for OptionContract {
//     fn from(data: TickerData) -> OptionContract {
//         Self {
//             // strike: Some(data.strike.unwrap()),
//             ask_amount: Some(data.best_ask_amount),
//             ask_price: data.best_ask_price,
//             ask_iv: data.ask_iv,
//             bid_amount: Some(data.best_bid_amount),
//             bid_price: data.best_bid_price,
//             bid_iv: data.bid_iv,
//             mark_price: Some(data.mark_price),
//             mark_iv: data.mark_iv,
//             // option_type: Some(option_type),
//             instrument_name: Some(data.instrument_name),
//             // expiration_timestamp: Some(data.expiration_timestamp),
//             underlying_index: data.underlying_index,
//             underlying_price: data.underlying_price,
//             greeks: data.greeks,
//             timestamp: Some(data.timestamp),
//         }
//     }
// }
