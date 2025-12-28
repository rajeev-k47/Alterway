use crate::config::Config;
use anyhow::{Result, anyhow};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

const BUFFER_SIZE: usize = 8192;

pub async fn handle_client(mut stream: TcpStream, config: Config) -> Result<()> {
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
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(anyhow!("Request timeout")),
    }
}
