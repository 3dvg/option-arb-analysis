use std::time::Instant;

use anyhow::{Error, Result};

use data_streamer::{OrbitCurrency, OrbitData, OrbitExchange, OrbitOrderbookStorage};
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
    // let exchanges = vec![OrbitExchange::Deribit];
    let currencies = vec![OrbitCurrency::Btc, OrbitCurrency::Eth, OrbitCurrency::Sol];
    // let currencies = vec![OrbitCurrency::Btc];
    let orbit_data = OrbitData::new(exchanges, currencies);
    debug!("orbit {:?}", orbit_data);

    let products = orbit_data.get_all_instruments().await?;
    info!("all products {:?}", products.len());
    let common_products = orbit_data.get_common_instruments().await?;
    info!("common products {:?}", common_products.len());

    let mut orbit_rx = orbit_data.consume_instruments(products.clone()).await?;

    let mut orbit_storage = OrbitOrderbookStorage::new(products.clone());
    info!("orbit_storage {:#?}", orbit_storage);

    let mut begin = Instant::now();
    let mut process_times = vec![];
    let mut event_times = vec![];

    let mut i = 0;
    while let Ok(event) = orbit_rx.recv().await {
        // info!("event {:?}", event);
        let elapsed = begin.elapsed().as_nanos();
        
        let begin2 = Instant::now();
        let storage = orbit_storage.process(event)?;
        let elapsed2 = begin2.elapsed().as_nanos();
        if i % 1000 == 0 {
            // info!("storage {:?}", storage);
            info!("({i}) orbit_rx queue {:?}, process time {elapsed2}ns, event time diff {elapsed}ns", orbit_rx.len());
        }
        process_times.push(elapsed2);
        event_times.push(elapsed);
        
        if i > 10_000 || orbit_rx.len() > 100_000 {break}
        i+=1;
        begin = Instant::now();
    }
    
    let process_avg = process_times.iter().sum::<u128>() as f64 / process_times.len() as f64;
    let event_avg = event_times.iter().sum::<u128>() as f64 / event_times.len() as f64;

    info!("sample of 10k or up to 100kclog...avg process time {process_avg}, avg time between events {event_avg}");
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
