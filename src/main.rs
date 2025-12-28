mod config;
mod handler;
mod parser;

use anyhow::Result;
use config::Config;
use log::error;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config = Config::from_file("config/proxy.toml").unwrap_or_else(|_| Config::default());
    let listener = TcpListener::bind(&config.listen_addr).await?;

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let config = config.clone();

                tokio::spawn(async move {
                    if let Err(e) = handler::handle_client(stream, addr, config).await {
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
