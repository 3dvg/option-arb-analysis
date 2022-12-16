use anyhow::{Error, Result};

use data_streamer::{OrbitData, OrbitExchange};
// use exchanges::delta::model::*;
// use exchanges::deribit::model::*;
use log::LevelFilter;
use log::*;

mod model;
// use model::{Exchange, OrbitData};
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
    // let deribit_cli = DeribitClient::new();
    // let deribit_products = deribit_cli.get_instruments().await?;
    // debug!("-- {:?}", deribit_products);
    // let mut deribit_rx = deribit_cli.consume(deribit_products).await?;
    // debug!("--");
    // while let Ok(event) = deribit_rx.recv().await {
    //     info!("{:?}", event);
    // info!("chanel {:?} ", deribit_rx.len());
    // }
    // consume_deribit(deribit_products).await;

    let exchanges = vec![OrbitExchange::Delta, OrbitExchange::Deribit];
    let orbit = OrbitData::new(exchanges);
    debug!("orbit {:?}", orbit);

    // let products = orbit.get_all_instruments().await?;
    // debug!("products {:?}", products.len());

    let products = orbit.get_common_instruments().await?;
    debug!("common products {:?}", products.len());
    // products.iter().for_each(|p| {
    //     debug!("== {p}");
    // });
    Ok(())
}

/*

let d = DateTime::parse_from_rfc3339("2023-01-06T12:00:00Z").unwrap().timestamp_millis();
    debug!("d {d}");



    let d: DateTime<Utc> = DateTime::from_utc(NaiveDateTime::from_timestamp_millis(d).unwrap(), Utc);
    debug!("d {d}");

    let d = d.date_naive();
    debug!("d {d}");


    let d = DateTime::parse_from_rfc3339("2023-01-06T12:00:00Z").unwrap().date_naive();
    debug!("d {d}");

    let t = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let a = NaiveDateTime::new(d, t);
    let d: DateTime<Utc> = DateTime::from_local(a, Utc);
    debug!("d {d}");

    let d = d.timestamp_millis();
    debug!("d {d}");
*/
