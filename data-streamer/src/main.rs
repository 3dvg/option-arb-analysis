use std::collections::HashSet;

use anyhow::{Error, Result};
// use data_streamer::{
//     consume_delta, consume_deribit, get_delta_products, get_deribit_products,
//     stream_websockets_delta,
// };

mod exchanges;
use exchanges::delta::model::*;
use exchanges::deribit::model::*;
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

    // info!("start exec");
    // let delta_cli = DeltaClient::new();
    // let delta_products = delta_cli.get_delta_products().await?;
    // let mut delta_rx = delta_cli.consume_delta(delta_products).await?;
    // while let Ok(event) = delta_rx.recv().await {
    //     info!("chanel {:?} ", delta_rx.len());
    // }
    // let maxclog = lens.iter().max();
    // info!("chanel {:?} -- lens {:?} -- max {:?}", delta_rx.len(), lens.len(), maxclog);
    // consume_delta(delta_products).await?;
    let deribit_cli = DeribitClient::new();
    let deribit_products = deribit_cli.get_instruments().await?;
    // debug!("-- {:?}", deribit_products);
    let mut deribit_rx = deribit_cli.consume(deribit_products).await?;
    debug!("--");
    while let Ok(event) = deribit_rx.recv().await {
        info!("{:?}", event);
        // info!("chanel {:?} ", deribit_rx.len());
    }
    // consume_deribit(deribit_products).await;

    let exchanges = vec![Exchange::Delta, Exchange::Deribit];
    let od = OrbitData::new(exchanges);
    // dbg!("od {:?}", od);

    Ok(())
}
