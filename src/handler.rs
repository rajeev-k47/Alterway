use crate::config::Config;
use crate::parser::HttpRequest;
use anyhow::{Result, anyhow};
use log::{error, info, warn};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

const BUFFER_SIZE: usize = 8192;

pub async fn handle_client(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    config: Config,
) -> Result<()> {
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut accumulated = Vec::new();

    let read_result = timeout(Duration::from_secs(config.request_timeout_secs), async {
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                return Err(anyhow!("Connection closed"));
            }
            accumulated.extend_from_slice(&buffer[..n]);
            if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if accumulated.len() > 1_000_000 {
                return Err(anyhow!("Request too large"));
            }
        }
        Ok(())
    })
    .await;

    match read_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(anyhow!("Request timeout")),
    }
    let (request, _) = match HttpRequest::parse(&accumulated) {
        Ok(req) => req,
        Err(e) => {
            warn!("Parse request {}: {}", client_addr, e);
            send_error_response(&mut stream, 400, "Bad Request").await?;
            return Ok(());
        }
    };

    info!(
        "Request {}: {} {} (Host: {}:{})",
        client_addr, request.method, request.uri, request.host, request.port
    );
    Ok(())
}

async fn send_error_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {} {}\n",
        status_code,
        status_text,
        status_text.len() + status_code.to_string().len() + 2,
        status_code,
        status_text
    );

    stream.write_all(response.as_bytes()).await?;
    Ok(())
}
