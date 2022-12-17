use std::collections::HashMap;
use std::hash::Hash;

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use log::debug;
use tokio::sync::broadcast::{self, Receiver, Sender};

mod exchanges;
use exchanges::delta::model::DeltaClient;
use exchanges::deribit::model::DeribitClient;

#[derive(Debug)]
pub struct OrbitData {
    pub exchanges: Vec<OrbitExchange>,
    pub clients: HashMap<OrbitExchange, OrbitExchangeClient>,
    pub sender: Sender<OrbitEvent>,
    pub receiver: Receiver<OrbitEvent>,
}

impl OrbitData {
    pub fn new(exchanges: Vec<OrbitExchange>) -> Self {
        let (sender, receiver) = broadcast::channel::<OrbitEvent>(10_000); //todo 10_000? check channel congestion
        let mut clients: HashMap<OrbitExchange, OrbitExchangeClient> =
            HashMap::with_capacity(exchanges.capacity());

        exchanges.iter().for_each(|exchange| match exchange {
            OrbitExchange::Delta => {
                clients.insert(
                    OrbitExchange::Delta,
                    OrbitExchangeClient::Delta(DeltaClient::new()), //todo pass sender here?
                );
            }
            OrbitExchange::Deribit => {
                clients.insert(
                    OrbitExchange::Deribit,
                    OrbitExchangeClient::Deribit(DeribitClient::new()), //todo pass sender here?
                );
            }
        });

        Self {
            exchanges,
            clients,
            sender,
            receiver,
        }
    }

    pub async fn get_all_instruments(
        &self,
    ) -> Result<HashMap<&OrbitExchange, Vec<OrbitInstrument>>, Error> {
        let mut instruments: HashMap<&OrbitExchange, Vec<OrbitInstrument>> =
            HashMap::with_capacity(self.clients.capacity());
        for (exchange, client) in self.clients.iter() {
            match client {
                OrbitExchangeClient::Delta(client) => {
                    let data = client.get_products().await?;
                    let orbit_data: Vec<OrbitInstrument> =
                        data.result.iter().map(OrbitInstrument::from).collect();
                    debug!(
                        "received {:?} instruments from {:?}",
                        orbit_data.len(),
                        exchange
                    );
                    instruments.insert(exchange, orbit_data);
                }
                OrbitExchangeClient::Deribit(client) => {
                    let data = client.get_instruments().await?;
                    let mut norm_deribit_instruments = vec![];
                    data.iter().for_each(|currency| {
                        let orbit_data: Vec<OrbitInstrument> =
                            currency.result.iter().map(OrbitInstrument::from).collect();
                        norm_deribit_instruments.push(orbit_data);
                    });
                    let flattened_norm_data: Vec<OrbitInstrument> =
                        norm_deribit_instruments.into_iter().flatten().collect();
                    debug!(
                        "received {:?} instruments from {:?}",
                        flattened_norm_data.len(),
                        exchange
                    );
                    instruments.insert(exchange, flattened_norm_data);
                }
            }
        }
        Ok(instruments)
    }

    pub async fn get_common_instruments(&self) -> Result<Vec<OrbitInstrument>, Error> {
        let instruments_map: HashMap<&OrbitExchange, Vec<OrbitInstrument>> =
            self.get_all_instruments().await?;
        let mut map: HashMap<String, Vec<OrbitInstrument>> = HashMap::new();
        for (_exchange, instruments) in instruments_map.iter() {
            instruments.iter().for_each(|x| {
                map.entry(x.get_cmp_code())
                    .and_modify(|list| list.push(x.clone()))
                    .or_insert_with(|| vec![x.clone()]);
            });
        }
        map.retain(|_k, v| v.len() == instruments_map.len());
        let result = map
            .into_values()
            .flatten()
            .collect::<Vec<OrbitInstrument>>();
        Ok(result)
    }

    pub fn consume_all_instruments() {
        todo!()
    }

    pub async fn consume_instruments(
        &self,
        symbols: Vec<OrbitInstrument>,
    ) -> Result<Receiver<OrbitEvent>, Error> {
        for (_exchange, client) in self.clients.iter() {
            match client {
                OrbitExchangeClient::Delta(client) => {
                    client.consume(self.sender.clone(), symbols.clone()).await?;
                }
                OrbitExchangeClient::Deribit(client) => {
                    client.consume(self.sender.clone(), symbols.clone()).await?;
                }
            }
        }
        Ok(self.sender.subscribe())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrbitInstrument {
    symbol: String,
    base: String,
    quote: String,
    strike: Option<u64>,
    expiration_datetime: Option<i64>, // datetime?
    expiration_date: Option<i64>,     // datetime?
    contract_type: OrbitContractType,
    exchange: OrbitExchange,
}

impl OrbitInstrument {
    pub fn get_cmp_code(&self) -> String {
        let d = match self.expiration_date {
            Some(date) => {
                let d: DateTime<Utc> =
                    DateTime::from_utc(NaiveDateTime::from_timestamp_millis(date).unwrap(), Utc);
                Some(d)
            }
            None => None,
        };
        format!(
            "{:?}_{}_{:?}_{:?}",
            self.contract_type, self.base, d, self.strike
        )
    }
}
// this was built do do the common instrument algo, may not use it leaving it here in case i come back
// impl PartialEq for OrbitInstrument {
//     fn eq(&self, other: &Self) -> bool {
//         self.base == other.base
//             && self.quote == other.quote
//             && self.strike == other.strike
//             && self.expiration == other.expiration
//             && self.contract_type == other.contract_type
//     }
// }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OrbitContractType {
    Spot,
    Future,
    PerpetualFuture,
    CallOption,
    PutOption,
    MoveOption,
    FutureCombo,
    OptionCombo,
    Unimplemented,
}

#[derive(Debug)]
pub enum OrbitExchangeClient {
    Delta(DeltaClient),
    Deribit(DeribitClient),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OrbitExchange {
    Deribit,
    Delta,
}

#[derive(Clone, Debug)]
pub struct OrbitEvent {
    pub exchange: OrbitExchange, // enum?
    pub symbol: String,
    pub contract_type: OrbitContractType,   //enum
    pub payload: Option<OrbitEventPayload>, //option
}

impl OrbitEvent {
    pub fn new(
        exchange: OrbitExchange,
        symbol: String,
        contract_type: OrbitContractType,
        payload: Option<OrbitEventPayload>,
    ) -> Self {
        Self {
            exchange,
            symbol,
            contract_type,
            payload,
        }
    }
}

#[derive(Clone, Debug)]
pub enum OrbitEventPayload {
    // OrderbookSnapshot(OrderbookUpdate),
    OrderbookUpdate(OrderbookUpdate),
}

// orderbook snapshots are just orderbooks updates with
// more levels and all them are "New" type
#[derive(Clone, Debug)]
pub struct OrderbookUpdate {
    pub timestamp: u64,
    pub bids: Vec<OrderbookUpdateLevel>,
    pub asks: Vec<OrderbookUpdateLevel>,
}

pub type Price = f64;
pub type Amount = f64;

#[derive(Clone, Debug)]
pub struct OrderbookUpdateLevel(pub OrderbookUpdateType, pub Price, pub Amount);

#[derive(Clone, Debug)]
pub enum OrderbookUpdateType {
    New,
    Change,
    Delete,
}
