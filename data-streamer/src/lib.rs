use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;
use std::time::Instant;

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use log::debug;
use ordered_float::OrderedFloat;
use tokio::sync::broadcast::{self, Receiver, Sender};

mod exchanges;
use exchanges::delta::model::DeltaClient;
use exchanges::deribit::model::DeribitClient;
use tracing_subscriber::field::debug;
use uuid::Uuid;

#[derive(Debug)]
pub struct OrbitData {
    pub exchanges: Vec<OrbitExchange>,
    pub currencies: Vec<OrbitCurrency>,
    pub clients: HashMap<OrbitExchange, OrbitExchangeClient>,
    pub sender: Sender<OrbitEvent>,
    pub receiver: Receiver<OrbitEvent>,
}

impl OrbitData {
    pub fn new(exchanges: Vec<OrbitExchange>, currencies: Vec<OrbitCurrency>) -> Self {
        let (sender, receiver) = broadcast::channel::<OrbitEvent>(250_000); //todo 10_000? check channel congestion
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
            currencies,
            clients,
            sender,
            receiver,
        }
    }

    pub async fn get_all_instruments_raw(
        &self,
    ) -> Result<HashMap<&OrbitExchange, Vec<OrbitInstrument>>, Error> {
        // i dont think returning hashmap is needed
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

        // Only use instruments we are interested in BTC ETH SOL; TODO optimize this with filter or fold, not critical
        let mut result = HashMap::with_capacity(instruments.capacity());
        for (k, v) in instruments.iter_mut() {
            let mut a = vec![];

            v.iter().for_each(|x| {
                self.currencies.iter().for_each(|c| {
                    if x.base == *c {
                        a.push(x.clone());
                    }
                })
            });

            result.insert(k.clone(), a);
        }

        Ok(result)
    }

    pub async fn get_all_instruments(&self) -> Result<Vec<OrbitInstrument>, Error> {
        let instruments_map: HashMap<&OrbitExchange, Vec<OrbitInstrument>> =
            self.get_all_instruments_raw().await?;
        let result: Vec<OrbitInstrument> = instruments_map.into_values().flatten().collect();
        Ok(result)
    }

    // todo error handling, what happens if common instruments number > total instruments for any of the exchanges etc
    pub async fn get_common_instruments(&self) -> Result<Vec<OrbitInstrument>, Error> {
        let instruments_map: HashMap<&OrbitExchange, Vec<OrbitInstrument>> =
            self.get_all_instruments_raw().await?;
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

#[derive(Clone, Debug)]
pub struct OrbitInstrument {
    symbol: String,
    base: OrbitCurrency,
    quote: OrbitCurrency,
    strike: Option<u64>,
    expiration_datetime: Option<DateTime<Utc>>, // datetime?todo
    expiration_date: Option<DateTime<Utc>>,     // datetime?
    contract_type: OrbitContractType,
    exchange: OrbitExchange,
}

impl OrbitInstrument {
    pub fn get_cmp_code(&self) -> String {
        let d = match self.expiration_date {
            Some(date) => {
                let d: DateTime<Utc> = date;
                    // DateTime::from_utc(NaiveDateTime::from_timestamp_millis(date).unwrap(), Utc);
                Some(d)
            }
            None => None,
        };
        format!(
            "{:?}_{:?}_{:?}_{:?}",
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
pub struct OrbitOrderbookStorage {
    pub id: Uuid,
    pub storage: BTreeMap<(OrbitExchange, OrbitCurrency), [Option<OrbitContractTypeOrderbook>; 3]>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrbitOrderbookStorage {
    pub fn new(instruments: Vec<OrbitInstrument>) -> Self {
        let mut storage: BTreeMap<
            (OrbitExchange, OrbitCurrency),
            [Option<OrbitContractTypeOrderbook>; 3],
        > = BTreeMap::new();

        for (i, instrument) in instruments.iter().enumerate() {
            // if i > 20 {
            //     break;
            // }
            // debug!("");
            // debug!("---- instrument {:?}", instrument);
            // if let Some(expiration) = instrument.expiration_date {
            //     d.insert(expiration);
            // }
            storage
                .entry((instrument.exchange.clone(), instrument.base.clone()))
                .and_modify(|contract_types| match instrument.contract_type {
                    OrbitContractType::Future => {
                        if let Some(expiration) = instrument.expiration_date {
                            if let Some(OrbitContractTypeOrderbook::Future(future_orderbook)) =
                                &mut contract_types[0]
                            {
                                debug!("future orderbook exists, inserting expiration and empty OB");
                                future_orderbook.insert(expiration, OrbitStorageOrderbook::default());
                            } else {
                                debug!("future doesnt exist, creating it and inserting expiration and empty OB");
                                let mut future_orderbook: BTreeMap<DateTime<Utc>, OrbitStorageOrderbook> = BTreeMap::new();
                                future_orderbook.insert(expiration, OrbitStorageOrderbook::default());
                                contract_types[0] = Some(OrbitContractTypeOrderbook::Future(future_orderbook));
                            }
                        }
                    }
                    OrbitContractType::CallOption | OrbitContractType::PutOption => {
                        if let Some(expiration) = instrument.expiration_date {
                            if let Some(strike) = instrument.strike {
                                if let Some(OrbitContractTypeOrderbook::Option(option_orderbook)) =
                                    &mut contract_types[1]
                                {
                                    option_orderbook.get_mut(&expiration).map(|inner| {
                                        inner
                                            .insert(strike, OrbitStorageOptionOrderbook::default());
                                    });
                                    debug!("option exists, inserting new expiration: {}", expiration);
                                    if let Some(inner) = option_orderbook.get_mut(&expiration) {
                                        debug!("expiration exists");
                                        let res = inner.insert(strike, OrbitStorageOptionOrderbook::default());
                                        debug!("inserted : {:?}", res);
                                    }else {
                                        debug!("cant find this expiration");
                                        let mut inner = BTreeMap::new();
                                        inner.insert(strike, OrbitStorageOptionOrderbook::default());
                                        option_orderbook.insert(expiration, inner);
                                    };  

                                } else {
                                    let mut inner = BTreeMap::new();
                                    inner.insert(strike, OrbitStorageOptionOrderbook::default());

                                    let mut outter = BTreeMap::new();
                                    outter.insert(expiration, inner);

                                    contract_types[1] = Some(OrbitContractTypeOrderbook::Option(outter));
                                }
                            }
                        }
                    }
                    OrbitContractType::PerpetualFuture => {
                        contract_types[2] = Some(OrbitContractTypeOrderbook::Perpetual(
                            OrbitStorageOrderbook::default(),
                        ));
                    }
                    _ => {}
                })
                .or_insert({
                    let mut contract_types = [None, None, None]; // Futures, Options, Perpetuals
                    match instrument.contract_type {
                        OrbitContractType::Future => {
                            if let Some(expiration) = instrument.expiration_date {
                                debug!("inserting futures orderbook");
                                let mut future_orderbook: BTreeMap<DateTime<Utc>, OrbitStorageOrderbook> = BTreeMap::new();
                                future_orderbook.insert(expiration, OrbitStorageOrderbook::default());
                                contract_types[0] = Some(OrbitContractTypeOrderbook::Future(future_orderbook));
                            }
                        }
                        OrbitContractType::CallOption | OrbitContractType::PutOption => {
                            if let Some(expiration) = instrument.expiration_date {
                                if let Some(strike) = instrument.strike {
                                    debug!("inserting options orderbook");
                                    let mut inner = BTreeMap::new();
                                    inner.insert(strike, OrbitStorageOptionOrderbook::default());
                                    
                                    let mut outter = BTreeMap::new();
                                    outter.insert(expiration, inner);
                                    
                                    contract_types[1] = Some(OrbitContractTypeOrderbook::Option(outter));
                                }
                            }
                        }
                        OrbitContractType::PerpetualFuture => {
                            debug!("inserting perps orderbook");
                            contract_types[2] = Some(OrbitContractTypeOrderbook::Perpetual(
                                OrbitStorageOrderbook::default(),
                            ));
                        }
                        _ => {
                            debug!("found unimplemented contract type {:?}", instrument);
                            continue;
                        }
                    }
                    contract_types
                });
            }
        Self {
            id: Uuid::new_v4(),
            storage,
            created_at: chrono::offset::Utc::now(),
            updated_at: chrono::offset::Utc::now(),
        }
    }
    
    pub fn process(&mut self, event: OrbitEvent) -> Result<BTreeMap< (OrbitExchange, OrbitCurrency),  [Option<OrbitContractTypeOrderbook>; 3]>, Error> {
        let k = (event.exchange, event.currency.unwrap());
        let contract_type = self.storage.get_mut(&k).unwrap(); // static sized array, we know length and items beforehand
        match event.contract_type.expect("This should unwrap fine") {
            OrbitContractType::Future => {
                if let Some(OrbitContractTypeOrderbook::Future(orderbook)) = &mut contract_type[0] {
                    // match &mut contract_type[0] {
                    // OrbitContractTypeOrderbook::Future(orderbook) => {
                    let k = event
                        .expiration
                        .expect("futures should always have expiration");
                    orderbook.get_mut(&k).map(|orbit_orderbook| {
                        event.payload.map(|p| match p {
                            OrbitEventPayload::OrderbookUpdate(event_orderbook) => {
                                event_orderbook.asks.iter().for_each(|ask| {
                                    match ask.0 {
                                        OrderbookUpdateType::New => {
                                            orbit_orderbook.asks.insert(OrderedFloat(ask.1), ask.2);
                                        }
                                        OrderbookUpdateType::Change => {
                                            orbit_orderbook.asks.insert(OrderedFloat(ask.1), ask.2);
                                            // todo, check entry.and modify....
                                        }
                                        OrderbookUpdateType::Delete => {
                                            orbit_orderbook.asks.remove(&OrderedFloat(ask.1));
                                        }
                                    }
                                });

                                event_orderbook.bids.iter().for_each(|bid| {
                                    match bid.0 {
                                        OrderbookUpdateType::New => {
                                            orbit_orderbook.bids.insert(OrderedFloat(bid.1), bid.2);
                                        }
                                        OrderbookUpdateType::Change => {
                                            orbit_orderbook.bids.insert(OrderedFloat(bid.1), bid.2);
                                            // todo, check entry.and modify....
                                        }
                                        OrderbookUpdateType::Delete => {
                                            orbit_orderbook.bids.remove(&OrderedFloat(bid.1));
                                        }
                                    }
                                });
                            }
                        })
                    });
                    // }
                }
                // _ => {}
                // }
            }
            OrbitContractType::CallOption => {
                if let Some(OrbitContractTypeOrderbook::Option(orderbook)) = &mut contract_type[1] {
                    let k = event
                        .expiration
                        .expect("options should always have expiration");
                    orderbook.get_mut(&k).map(|expiration| {
                        let k = event.strike.expect("options should always have strike");
                        expiration.get_mut(&k).map(|orbit_option_orderbook| {
                            event.payload.map(|p| match p {
                                OrbitEventPayload::OrderbookUpdate(event_orderbook) => {
                                    event_orderbook.asks.iter().for_each(|ask| {
                                        match ask.0 {
                                            OrderbookUpdateType::New => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .asks
                                                    .insert(OrderedFloat(ask.1), ask.2);
                                            }
                                            OrderbookUpdateType::Change => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .asks
                                                    .insert(OrderedFloat(ask.1), ask.2);
                                                // todo, check entry.and modify....
                                            }
                                            OrderbookUpdateType::Delete => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .asks
                                                    .remove(&OrderedFloat(ask.1));
                                            }
                                        }
                                    });
                                    event_orderbook.bids.iter().for_each(|bid| {
                                        match bid.0 {
                                            OrderbookUpdateType::New => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .bids
                                                    .insert(OrderedFloat(bid.1), bid.2);
                                            }
                                            OrderbookUpdateType::Change => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .bids
                                                    .insert(OrderedFloat(bid.1), bid.2);
                                                // todo, check entry.and modify....
                                            }
                                            OrderbookUpdateType::Delete => {
                                                orbit_option_orderbook
                                                    .calls
                                                    .bids
                                                    .remove(&OrderedFloat(bid.1));
                                            }
                                        }
                                    });
                                }
                            });
                        });
                    });
                }
            }
            OrbitContractType::PutOption => {
                if let Some(OrbitContractTypeOrderbook::Option(orderbook)) = &mut contract_type[1] {
                    let k = event
                        .expiration
                        .expect("options should always have expiration");
                    orderbook.get_mut(&k).map(|expiration| {
                        let k: u64 = event.strike.expect("options should always have strike");
                        expiration.get_mut(&k).map(|orbit_option_orderbook| {
                            event.payload.map(|p| match p {
                                OrbitEventPayload::OrderbookUpdate(event_orderbook) => {
                                    event_orderbook.asks.iter().for_each(|ask| {
                                        match ask.0 {
                                            OrderbookUpdateType::New => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .asks
                                                    .insert(OrderedFloat(ask.1), ask.2);
                                            }
                                            OrderbookUpdateType::Change => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .asks
                                                    .insert(OrderedFloat(ask.1), ask.2);
                                                // todo, check entry.and modify....
                                            }
                                            OrderbookUpdateType::Delete => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .asks
                                                    .remove(&OrderedFloat(ask.1));
                                            }
                                        }
                                    });
                                    event_orderbook.bids.iter().for_each(|bid| {
                                        match bid.0 {
                                            OrderbookUpdateType::New => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .bids
                                                    .insert(OrderedFloat(bid.1), bid.2);
                                            }
                                            OrderbookUpdateType::Change => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .bids
                                                    .insert(OrderedFloat(bid.1), bid.2);
                                                // todo, check entry.and modify....
                                            }
                                            OrderbookUpdateType::Delete => {
                                                orbit_option_orderbook
                                                    .puts
                                                    .bids
                                                    .remove(&OrderedFloat(bid.1));
                                            }
                                        }
                                    });
                                }
                            });
                        });
                    });
                }
            }
            OrbitContractType::PerpetualFuture => {
                if let Some(OrbitContractTypeOrderbook::Perpetual(orbit_orderbook)) =
                    &mut contract_type[2]
                {
                    if let Some(OrbitEventPayload::OrderbookUpdate(event_orderbook)) = event.payload
                    {
                        event_orderbook.asks.iter().for_each(|ask| {
                            match ask.0 {
                                OrderbookUpdateType::New => {
                                    orbit_orderbook.asks.insert(OrderedFloat(ask.1), ask.2);
                                }
                                OrderbookUpdateType::Change => {
                                    orbit_orderbook.asks.insert(OrderedFloat(ask.1), ask.2);
                                    // todo, check entry.and modify....
                                }
                                OrderbookUpdateType::Delete => {
                                    orbit_orderbook.asks.remove(&OrderedFloat(ask.1));
                                }
                            }
                        });

                        event_orderbook.bids.iter().for_each(|bid| {
                            match bid.0 {
                                OrderbookUpdateType::New => {
                                    orbit_orderbook.bids.insert(OrderedFloat(bid.1), bid.2);
                                }
                                OrderbookUpdateType::Change => {
                                    orbit_orderbook.bids.insert(OrderedFloat(bid.1), bid.2);
                                    // todo, check entry.and modify....
                                }
                                OrderbookUpdateType::Delete => {
                                    orbit_orderbook.bids.remove(&OrderedFloat(bid.1));
                                }
                            }
                        });
                    };
                }
            }
            _ => {}
        }
        Ok(self.storage.clone())
    }
}

#[derive(Clone, Debug)]
pub enum OrbitContractTypeOrderbook {
    Future(OrbitFutureOrderbook),
    Option(OrbitOptionOrderbook),
    Perpetual(OrbitPerpetualOrderbook),
}

pub type Expiration = DateTime<Utc>;
pub type Strike = u64;
pub type OrbitPerpetualOrderbook = OrbitStorageOrderbook;
pub type OrbitFutureOrderbook = BTreeMap<Expiration, OrbitStorageOrderbook>;
pub type OrbitOptionOrderbook = BTreeMap<Expiration, BTreeMap<Strike, OrbitStorageOptionOrderbook>>;

#[derive(Clone, Debug)]
pub struct OrbitOrderbook {
    id: Uuid,
    timestamp: i64,
    bids: Vec<OrderbookUpdateLevel>,
    asks: Vec<OrderbookUpdateLevel>,
}

pub type OrbitOrderbookPrice = OrderedFloat<f64>;
pub type OrbitOrderbookAmount = f64;
#[derive(Clone, Debug, Default)]
pub struct OrbitStorageOrderbook {
    id: Uuid,
    timestamp: i64,
    bids: BTreeMap<OrbitOrderbookPrice, OrbitOrderbookAmount>,
    asks: BTreeMap<OrbitOrderbookPrice, OrbitOrderbookAmount>,
}

#[derive(Clone, Debug, Default)]
pub struct OrbitStorageOptionOrderbook {
    puts: OrbitStorageOrderbook,
    calls: OrbitStorageOrderbook,
}

#[derive(Debug)]
pub enum OrbitExchangeClient {
    Delta(DeltaClient),
    Deribit(DeribitClient),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrbitExchange {
    Deribit,
    Delta,
}

#[derive(Clone, Debug)]
pub struct OrbitEvent {
    pub exchange: OrbitExchange,
    pub symbol: String,
    pub currency: Option<OrbitCurrency>,
    pub contract_type: Option<OrbitContractType>,
    pub expiration: Option<DateTime<Utc>>,
    pub strike: Option<Strike>,
    pub payload: Option<OrbitEventPayload>,
}

impl OrbitEvent {
    pub fn new(
        exchange: OrbitExchange,
        symbol: String,
        currency: Option<OrbitCurrency>,
        contract_type: Option<OrbitContractType>,
        expiration: Option<DateTime<Utc>>,
        strike: Option<Strike>,
        payload: Option<OrbitEventPayload>,
    ) -> Self {
        Self {
            exchange,
            symbol,
            currency,
            contract_type,
            expiration,
            strike,
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrbitCurrency {
    Btc,
    Eth,
    Sol,
    Unimplemented,
}
