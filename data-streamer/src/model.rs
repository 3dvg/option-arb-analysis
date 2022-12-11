use std::collections::{BTreeMap, HashMap};
use tokio::sync::broadcast::{self, Receiver, Sender};

#[derive(Debug)]
pub struct OrbitData {
    pub subscriptions: Vec<Exchange>,
    pub sender: Sender<Event>,
    pub receiver: Receiver<Event>,
}

impl OrbitData {
    pub fn new(subscriptions: Vec<Exchange>) -> Self {
        let (sender, receiver) = broadcast::channel::<Event>(256); //todo 256? check channel congestion
        Self {
            subscriptions,
            sender,
            receiver,
        }
    }

    pub fn get_all_instruments() {
        todo!()
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

#[derive(Clone)]
pub enum Event {
    Orderbook,
}

pub struct Storage {
    pub exchange: HashMap<Exchange, StorageInstruments>,
}

#[derive(Debug)]
pub enum Exchange {
    Deribit,
    Delta,
}

pub type StorageInstruments = HashMap<InstrumentType, StorageData>;

pub enum InstrumentType {
    Future,
    Option,
    Perpetual,
}

pub enum StorageData {
    FuturesData, //TODO
    OptionsData(OptionInstrument),
    PerpetualsData,
}
// todo instrument -> optionchains
pub type Symbol = String;
pub type Expiration = u64;
pub type Strike = u64;
pub type OptionInstrument = HashMap<Symbol, OptionChains>;
pub type OptionChains = BTreeMap<Expiration, OptionChain>;
pub type OptionChain = BTreeMap<Strike, OptionChainLevel>;
pub struct OptionChainLevel {
    pub puts: Vec<OptionChainLevelOrderbook>,
    pub calls: Vec<OptionChainLevelOrderbook>,
}

pub struct OptionChainLevelOrderbook {
    pub bids: Vec<OptionChainLevelOrderbookLevel>,
    pub asks: Vec<OptionChainLevelOrderbookLevel>,
    pub sequence: u64,
    pub updated_at: u64,
    pub timestamp: u64,
    pub symbol: String,
}

pub struct OptionChainLevelOrderbookLevel {
    pub price: f64,
    pub size: f64,
}
