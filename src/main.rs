mod config;
mod filter;
mod handler;
mod logger;
mod parser;

use crate::filter::Filter;
use anyhow::Result;
use config::Config;
use log::{error, info};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config = Config::from_file("config/proxy.toml").unwrap_or_else(|_| Config::default());
    let listener = TcpListener::bind(&config.listen_addr).await?;

    let filter = Arc::new(Filter::from_file(&config.blocked_domains_file)?);
    info!("I[Starting proxy server] on {}", config.listen_addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let config = config.clone();
                let filter = Arc::clone(&filter);

                tokio::spawn(async move {
                    if let Err(e) = handler::handle_client(stream, addr, filter, config).await {
                        error!("E[Handling Client] {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                error!("E[Connection] : {}", e);
            }
        }
    }
}
