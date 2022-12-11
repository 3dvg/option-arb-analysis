use anyhow::{Error, Result};
use data_streamer::{
    consume_delta, consume_deribit, get_delta_products, get_deribit_products,
    stream_websockets_delta,
};

use log::LevelFilter;
use log::*;
use tokio;

mod model;
use model::{Exchange, OrbitData};
#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(LevelFilter::Debug)
        .init();

    info!("start exec");
    let delta_products =get_delta_products().await?;
    debug!("-- {:?}", delta_products);
    // consume_delta(delta_products).await?;
    // let deribit_products = get_deribit_products().await?;
    // debug!("-- {:?}", deribit_products);
    // consume_deribit(deribit_products).await;

    // let exchanges = vec![Exchange::Delta, Exchange::Deribit];
    // let od = OrbitData::new(exchanges);
    // dbg!("od {:?}", od);
    
    
    Ok(())
}
