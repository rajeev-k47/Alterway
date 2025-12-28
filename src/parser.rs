use anyhow::{Result, anyhow};
use httparse;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: u8,
    pub host: String,
    pub port: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn parse(buffer: &[u8]) -> Result<(Self, usize)> {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);

        let status = req
            .parse(buffer)
            .map_err(|e| anyhow!("E[Parse Request]: {:?}", e))?;

        let header_len = match status {
            httparse::Status::Complete(len) => len,
            httparse::Status::Partial => return Err(anyhow!("E[Incomplete request]")),
        };

        let method = req
            .method
            .ok_or_else(|| anyhow!("E[Missing method]"))?
            .to_string();

        let uri = req.path.ok_or_else(|| anyhow!("Missing path"))?.to_string();
        let version = req.version.ok_or_else(|| anyhow!("Missing version"))?;

        let mut headers_vec = Vec::new();
        let mut host = None;
        let mut content_length = 0;

        for header in req.headers {
            let name = header.name.to_string();
            let value = String::from_utf8_lossy(header.value).to_string();

            if name.eq_ignore_ascii_case("host") {
                host = Some(value.clone());
            } else if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }

            headers_vec.push((name, value));
        }

        let (host, port) = if uri.starts_with("http://") || uri.starts_with("https://") {
            parse_absolute_uri(&uri)?
        } else if let Some(h) = host {
            parse_host_header(&h)?
        } else {
            return Err(anyhow!("E[Missing host]"));
        };

        let body = if content_length > 0 && buffer.len() > header_len {
            let body_start = header_len;
            let body_end = std::cmp::min(body_start + content_length, buffer.len());
            buffer[body_start..body_end].to_vec()
        } else {
            Vec::new()
        };

        Ok((
            HttpRequest {
                method,
                uri,
                version,
                host,
                port,
                headers: headers_vec,
                body,
            },
            header_len + content_length,
        ))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let path = if self.uri.starts_with("http://") || self.uri.starts_with("https://") {
            extract_path(&self.uri).unwrap_or("/")
        } else {
            &self.uri
        };

        result.extend_from_slice(
            format!("{} {} HTTP/1.{}\r\n", self.method, path, self.version).as_bytes(),
        );
        for (name, value) in &self.headers {
            result.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
        }

        result.extend_from_slice(b"\r\n");
        if !self.body.is_empty() {
            result.extend_from_slice(&self.body);
        }

        result
    }
}

fn parse_absolute_uri(uri: &str) -> Result<(String, u16)> {
    let without_scheme = uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"))
        .ok_or_else(|| anyhow!(""))?;

    let parts: Vec<&str> = without_scheme.split('/').collect();
    let host_port = parts[0];

    parse_host_header(host_port)
}

fn parse_host_header(host: &str) -> Result<(String, u16)> {
    if let Some(colon_pos) = host.rfind(':') {
        let hostname = &host[..colon_pos];
        let port = host[colon_pos + 1..].parse().map_err(|_| anyhow!(""))?;
        Ok((hostname.to_string(), port))
    } else {
        Ok((host.to_string(), 80))
    }
}

fn extract_path(uri: &str) -> Option<&str> {
    let without_scheme = uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"))?;

    if let Some(slash_pos) = without_scheme.find('/') {
        Some(&without_scheme[slash_pos..])
    } else {
        Some("/")
    }
}
