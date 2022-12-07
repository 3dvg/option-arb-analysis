use anyhow::{Error, Result};
use data_streamer::{get_delta_products, consume_delta};
use log::LevelFilter;
use tokio;
use log::*;
#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::formatted_timed_builder()
    .filter_level(LevelFilter::Debug)
    .init();
    
    info!("start exec");
    get_delta_products().await;
    // consume_delta().await;
    Ok(())
}
