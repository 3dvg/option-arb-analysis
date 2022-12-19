use anyhow::{Error, Result};

use data_streamer::{OrbitData, OrbitExchange, OrbitOrderbookStorage};
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

    let exchanges = vec![OrbitExchange::Delta, OrbitExchange::Deribit];
    let orbit_data = OrbitData::new(exchanges);
    debug!("orbit {:?}", orbit_data);

    let products = orbit_data.get_common_instruments().await?;
    debug!("common products {:?}", products.len());

    let mut orbit_rx = orbit_data.consume_instruments(products).await?;

    let mut orbit_storage = OrbitOrderbookStorage::new();

    while let Ok(event) = orbit_rx.recv().await {
        info!("{:?}", event);
        // orbit_storage.process(event);
    }
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
