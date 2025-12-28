use crate::parser::HttpRequest;
use chrono::Local;
use log::info;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;

pub fn log_request(
    request: &HttpRequest,
    client_addr: SocketAddr,
    action: &str,
    #[allow(unused)] status: u16,
    bytes_transferred: usize,
) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

    let log_entry = format!(
        "[{}] {} | {} {} | {}:{} | {} | {} bytes",
        timestamp,
        client_addr,
        request.method,
        request.uri,
        request.host,
        request.port,
        action,
        bytes_transferred
    );

    info!("{}", log_entry);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/proxy.log")
    {
        writeln!(file, "{}", log_entry).ok();
    }
}
