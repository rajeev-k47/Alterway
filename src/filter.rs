use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;

#[derive(Debug)]
pub struct Filter {
    blocked_domains: HashSet<String>,
    blocked_ips: HashSet<IpAddr>,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            blocked_domains: HashSet::new(),
            blocked_ips: HashSet::new(),
        }
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let mut filter = Self::new();

        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Ok(ip) = line.parse::<IpAddr>() {
                    filter.blocked_ips.insert(ip); //for ips
                } else {
                    filter.blocked_domains.insert(line.to_lowercase()); //normalize to lowercase
                }
            }
        }

        Ok(filter)
    }

    pub fn is_blocked(&self, host: &str) -> bool {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return self.blocked_ips.contains(&ip);
        }
        let host_lower = host.to_lowercase();
        if self.blocked_domains.contains(&host_lower) {
            return true;
        }

        for blocked_domain in &self.blocked_domains {
            if host_lower.ends_with(blocked_domain) {
                if host_lower == *blocked_domain//for subdomain check
                    || host_lower.ends_with(&format!(".{}", blocked_domain))
                {
                    return true;
                }
            }
        }

        false
    }
}
