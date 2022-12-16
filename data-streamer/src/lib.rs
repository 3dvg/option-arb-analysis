use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::mem::swap;

use anyhow::Error;
use log::debug;
use serde::__private::de;
use tokio::sync::mpsc::{self, Receiver, Sender};

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
        let (sender, receiver) = mpsc::channel::<OrbitEvent>(10_000); //todo 10_000? check channel congestion
        let mut clients: HashMap<OrbitExchange, OrbitExchangeClient> =
            HashMap::with_capacity(exchanges.capacity());

        let _ = exchanges.iter().for_each(|exchange| match exchange {
            OrbitExchange::Delta => {
                clients.insert(
                    OrbitExchange::Delta,
                    OrbitExchangeClient::Delta(DeltaClient::new()),
                );
            }
            OrbitExchange::Deribit => {
                clients.insert(
                    OrbitExchange::Deribit,
                    OrbitExchangeClient::Deribit(DeribitClient::new()),
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
                    let orbit_data: Vec<OrbitInstrument> = data
                        .result
                        .iter()
                        .map(|product| OrbitInstrument::from(product))
                        .collect();
                    debug!(
                        "received {:?} instruments from {:?}",
                        orbit_data.len(),
                        exchange
                    );
                    instruments.insert(exchange, orbit_data);
                }
                OrbitExchangeClient::Deribit(client) => {
                    let data = client.get_instruments().await?;
                    let orbit_data: Vec<OrbitInstrument> = data
                        .result
                        .iter()
                        .map(|product| OrbitInstrument::from(product))
                        .collect();
                    debug!(
                        "received {:?} instruments from {:?}",
                        orbit_data.len(),
                        exchange
                    );
                    instruments.insert(exchange, orbit_data);
                }
            }
        }
        Ok(instruments)
    }

    pub async fn get_common_instruments(&self) -> Result<Vec<String>, Error> {
        let instruments_map: HashMap<&OrbitExchange, Vec<OrbitInstrument>> =
            self.get_all_instruments().await?;
        // let codes_map: HashMap<OrbitExchange, Vec<String>>
        let mut map: HashMap<String, &OrbitInstrument> = HashMap::new();
        let mut result = vec![];
        let mut debugging = vec![];
        for (i, (exchange, instruments)) in instruments_map.iter().enumerate() {
            if i == 0 {
                let orbit_codes =
                    instruments
                        .iter()
                        .fold(HashMap::new(), |mut orbit_codes, instrument| {
                            orbit_codes.insert(instrument.get_cmp_code(), instrument);
                            orbit_codes
                        });
                map = orbit_codes;

                let dev =
                    instruments
                        .iter()
                        .fold(Vec::new(), |mut orbit_codes, instrument| {
                            orbit_codes.push(instrument.get_cmp_code());
                            orbit_codes
                        });
                debugging.push(dev);
                continue;
            }
            debug!("2nd round {:?}", exchange);
            let dev =
            instruments
                .iter()
                .fold(Vec::new(), |mut orbit_codes, instrument| {
                    orbit_codes.push(instrument.get_cmp_code());
                    orbit_codes
                });
        debugging.push(dev);
        }

        // debug!("debugging {:?}", debugging);
        debugging[0].sort(); 
        debugging[1].sort();

        for (i, j) in debugging[0].iter().zip(debugging[1].iter()) {
            debug!("{i}||{j}");
        }
        Ok(result)
    }

    // fn _find_common_elements(
    //     list1: Vec<OrbitInstrument>,
    //     list2: Vec<OrbitInstrument>,
    // ) -> Result<Vec<OrbitInstrument>, Error> {
    //     if list1.len() > list2.len() {
    //         swap(&mut list1, &mut list2)
    //     }
    //     list1.iter().fold(Vec<OrbitInstrument>::new(), );

    //     Ok(list1)
    // }

    pub fn consume_all_instruments() {
        todo!()
    }

    pub fn consume_instruments(symbols: Vec<String>) {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OrbitInstrument {
    symbol: String,
    base: String,
    quote: String,
    strike: Option<u64>,
    expiration_datetime: Option<i64>, // datetime?
    expiration_date: Option<i64>, // datetime?
    contract_type: OrbitContractType,
    exchange: OrbitExchange,
}

impl OrbitInstrument {
    pub fn get_cmp_code(&self) -> String {
        format!(
            "{:?}.{}.{:?}.{:?}.",
            self.contract_type, self.base, self.expiration_date, self.strike
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

#[derive(Debug, PartialEq, Eq, Hash)]
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

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum OrbitExchange {
    Deribit,
    Delta,
}

#[derive(Clone, Debug)]
pub struct OrbitEvent {
    pub exchange: String, // enum?
    pub symbol: String,
    pub contract_type: String,      //enum
    pub payload: OrbitEventPayload, //option
}

impl OrbitEvent {
    pub fn new(
        exchange: String,
        symbol: String,
        contract_type: String,
        payload: OrbitEventPayload,
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
