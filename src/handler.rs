use crate::config::Config;
use crate::filter::Filter;
use crate::logger::log_request;
use crate::parser::HttpRequest;
use anyhow::{Result, anyhow};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

const BUFFER_SIZE: usize = 8192;

pub async fn handle_client(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    filter: Arc<Filter>,
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

    if filter.is_blocked(&request.host) {
        warn!("Blocked request to {} from {}", request.host, client_addr);
        log_request(&request, client_addr, "BLOCKED", 403, 0);
        send_error_response(&mut stream, 403, "Forbidden").await?;
        return Ok(());
    }
    if request.method.eq_ignore_ascii_case("CONNECT") {
        return handle_connect(stream, request, client_addr, config).await;
    }

    forward_http_request(stream, request, client_addr, config).await
}

async fn forward_http_request(
    mut client_stream: TcpStream,
    request: HttpRequest,
    client_addr: SocketAddr,
    config: Config,
) -> Result<()> {
    let target_addr = format!("{}:{}", request.host, request.port);

    let mut server_stream = match timeout(
        Duration::from_secs(config.request_timeout_secs),
        TcpStream::connect(&target_addr),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            error!("E[Failed to connect] to {}: {}", target_addr, e);
            send_error_response(&mut client_stream, 502, "Bad Gateway").await?;
            return Ok(());
        }
        Err(_) => {
            error!("E[Connection timeout] to {}", target_addr);
            send_error_response(&mut client_stream, 504, "Gateway Timeout").await?;
            return Ok(());
        }
    };

    let request_bytes = request.to_bytes();
    server_stream.write_all(&request_bytes).await?;

    let mut total_bytes = 0;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    //back to client
    loop {
        let n = match server_stream.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                error!("E[Error: {}]", e);
                break;
            }
        };

        if let Err(e) = client_stream.write_all(&buffer[..n]).await {
            error!("E[Error: {}]", e);
            break;
        }

        total_bytes += n;
    }

    log_request(&request, client_addr, "ALLOWED", 200, total_bytes);
    info!(
        "Completed request from {} ({} bytes)",
        client_addr, total_bytes
    );

    Ok(())
}

async fn handle_connect(
    mut client_stream: TcpStream,
    request: HttpRequest,
    client_addr: SocketAddr,
    config: Config,
) -> Result<()> {
    let target_addr = format!("{}:{}", request.host, request.port);
    info!("Tunnel to {} from {}", target_addr, client_addr);

    let mut server_stream = match timeout(
        Duration::from_secs(config.request_timeout_secs),
        TcpStream::connect(&target_addr),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            error!("E[Failed to connect] to {}: {}", target_addr, e);
            send_error_response(&mut client_stream, 502, "Bad Gateway").await?;
            return Ok(());
        }
        Err(_) => {
            error!("E[Timeout] to {}", target_addr);
            send_error_response(&mut client_stream, 504, "Gateway Timeout").await?;
            return Ok(());
        }
    };

    client_stream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    let (mut client_read, mut client_write) = client_stream.split();
    let (mut server_read, mut server_write) = server_stream.split();

    let client_to_server = async {
        let mut buf = vec![0u8; BUFFER_SIZE];
        loop {
            match client_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if server_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let server_to_client = async {
        let mut buf = vec![0u8; BUFFER_SIZE];
        loop {
            match server_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if client_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    tokio::select! {
        _ = client_to_server => {},
        _ = server_to_client => {},
    }

    log_request(&request, client_addr, "CONNECT", 200, 0);
    info!("tunnel closed for {}", client_addr);

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
