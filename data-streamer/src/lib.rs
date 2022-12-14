use std::collections::HashMap;

use anyhow::Error;
use log::debug;
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
                    debug!("received {:?} instruments from {:?}", orbit_data.len(), exchange);
                    instruments.insert(exchange, orbit_data);
                }
                OrbitExchangeClient::Deribit(client) => {
                    let data = client.get_instruments().await?;
                    let orbit_data: Vec<OrbitInstrument> = data
                        .result
                        .iter()
                        .map(|product| OrbitInstrument::from(product))
                        .collect();
                    debug!("received {:?} instruments from {:?}", orbit_data.len(), exchange);
                    instruments.insert(exchange, orbit_data);
                }
            }
        }
        Ok(instruments)
    }

    pub fn get_common_instruments() {
        todo!()
    }

    pub fn consume_all_instruments() {
        todo!()
    }

    pub fn consume_instruments(symbols: Vec<String>) {
        todo!()
    }
}

#[derive(Debug)]
pub struct OrbitInstrument {
    symbol: String,
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
