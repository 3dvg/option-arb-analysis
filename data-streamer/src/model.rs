
pub struct Storage {
    pub exchange: HashMap<Exchange, StorageInstruments>
}

pub enum Exchange {
    Delta, 
    Deribit
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

pub type OptionInstrument = HashMap<Symbol, OptionChains>;
pub type OptionChains = BtreeMap<Expiration, OptionChain>;
pub type OptionChain = BtreeMap<Strike, OptionChainLevel>;
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