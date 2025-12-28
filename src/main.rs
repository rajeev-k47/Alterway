mod config;
mod handler;

use anyhow::Result;
use config::Config;
use std::net::TcpListener;

fn main() -> Result<()> {
    env_logger::init();

    let config = Config::from_file("config/proxy.toml").unwrap_or_else(|_| Config::default());
    let listener = TcpListener::bind("127.0.0.1:8080")?;

    for stream in listener.incoming() {
        handler::handle_client(stream?);
    }
    Ok(())
}
