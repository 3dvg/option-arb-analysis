
pub struct Storage {
    pub exchange: HashMap<Exchange, OptionChains>
}

pub enum Exchange {
    Delta, 
    Deribit
}

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