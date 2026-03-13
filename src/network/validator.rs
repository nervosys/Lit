use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use super::audit::AuditLog;

/// Network configuration for LAN restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Allowed IP networks in CIDR notation
    pub allowed_networks: Vec<String>,

    /// Allowed hostnames/domains
    pub allowed_hosts: Vec<String>,

    /// Enable audit logging
    pub audit_log: bool,

    /// Audit log path
    pub audit_log_path: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            allowed_networks: vec![
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
            ],
            allowed_hosts: vec![],
            audit_log: true,
            audit_log_path: Some("~/.lit/audit.log".to_string()),
        }
    }
}

impl NetworkConfig {
    /// Load configuration from file
    pub fn load() -> Result<Self, String> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(NetworkConfig::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
    }

    /// Get the config file path
    fn config_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".litconfig"))
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), String> {
        let config_path = Self::config_path()?;

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&config_path, content).map_err(|e| format!("Failed to write config: {}", e))
    }
}

/// URL validator for LAN-only access
pub struct NetworkValidator {
    config: NetworkConfig,
}

impl NetworkValidator {
    /// Create a new validator
    pub fn new() -> Result<Self, String> {
        let config = NetworkConfig::load()?;
        Ok(NetworkValidator { config })
    }

    /// Validate a URL for LAN access
    pub fn validate_url(&self, url: &str) -> Result<(), String> {
        // Parse the URL
        let url_parts = self.parse_url(url)?;

        // Check protocol
        if url_parts.protocol != "lit" {
            return Err(format!(
                "Invalid protocol '{}'. Only 'lit://' protocol is allowed for LAN operations",
                url_parts.protocol
            ));
        }

        // Validate host
        self.validate_host(&url_parts.host)?;

        // Log if enabled
        if self.config.audit_log {
            self.log_access(url)?;
        }

        Ok(())
    }

    /// Parse a URL into components
    fn parse_url(&self, url: &str) -> Result<UrlParts, String> {
        let re =
            Regex::new(r"^([a-z]+)://([^/]+)(.*)$").map_err(|e| format!("Regex error: {}", e))?;

        let captures = re
            .captures(url)
            .ok_or_else(|| format!("Invalid URL format: {}", url))?;

        Ok(UrlParts {
            protocol: captures.get(1).unwrap().as_str().to_string(),
            host: captures.get(2).unwrap().as_str().to_string(),
            path: captures.get(3).unwrap().as_str().to_string(),
        })
    }

    /// Validate a host against the whitelist
    fn validate_host(&self, host: &str) -> Result<(), String> {
        // Check if it's an IP address
        if let Ok(ip) = host.parse::<IpAddr>() {
            return self.validate_ip(&ip);
        }

        // Check against allowed hosts
        if self.config.allowed_hosts.iter().any(|h| h == host) {
            return Ok(());
        }

        // Try to resolve hostname to IP
        // For now, we'll just check if it matches the allowed hosts
        // In a real implementation, you would resolve DNS here

        Err(format!(
            "Host '{}' is not in the allowed LAN hosts list. \
             Configure allowed hosts in ~/.litconfig",
            host
        ))
    }

    /// Validate an IP address against allowed networks
    fn validate_ip(&self, ip: &IpAddr) -> Result<(), String> {
        match ip {
            IpAddr::V4(ipv4) => self.validate_ipv4(ipv4),
            IpAddr::V6(_) => Err("IPv6 not supported yet".to_string()),
        }
    }

    /// Validate an IPv4 address against CIDR ranges
    fn validate_ipv4(&self, ip: &Ipv4Addr) -> Result<(), String> {
        for network in &self.config.allowed_networks {
            if self.ip_in_cidr(ip, network)? {
                return Ok(());
            }
        }

        Err(format!(
            "IP address '{}' is not in any allowed LAN network range. \
             Configure allowed networks in ~/.litconfig",
            ip
        ))
    }

    /// Check if an IP is in a CIDR range
    fn ip_in_cidr(&self, ip: &Ipv4Addr, cidr: &str) -> Result<bool, String> {
        let parts: Vec<&str> = cidr.split('/').collect();

        if parts.len() != 2 {
            return Err(format!("Invalid CIDR notation: {}", cidr));
        }

        let network_ip: Ipv4Addr = parts[0]
            .parse()
            .map_err(|e| format!("Invalid IP in CIDR: {}", e))?;

        let prefix_len: u8 = parts[1]
            .parse()
            .map_err(|e| format!("Invalid prefix length in CIDR: {}", e))?;

        if prefix_len > 32 {
            return Err("Invalid prefix length: must be 0-32".to_string());
        }

        let mask = if prefix_len == 0 {
            0u32
        } else {
            !0u32 << (32 - prefix_len)
        };

        let network_int = u32::from_be_bytes(network_ip.octets());
        let ip_int = u32::from_be_bytes(ip.octets());

        Ok((network_int & mask) == (ip_int & mask))
    }

    /// Log a network access attempt
    fn log_access(&self, url: &str) -> Result<(), String> {
        if self.config.audit_log {
            // Use HMAC-signed audit log
            let audit_path = self.config.audit_log_path.as_deref();
            let audit = AuditLog::new(audit_path)?;
            audit.log("NETWORK_ACCESS", url)?;
        }

        Ok(())
    }
}

#[allow(dead_code)]
struct UrlParts {
    protocol: String,
    host: String,
    path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cidr_matching() {
        let config = NetworkConfig::default();
        let validator = NetworkValidator { config };

        let ip: Ipv4Addr = "192.168.1.100".parse().unwrap();
        assert!(validator.ip_in_cidr(&ip, "192.168.0.0/16").unwrap());
        assert!(!validator.ip_in_cidr(&ip, "10.0.0.0/8").unwrap());
    }

    #[test]
    fn test_url_parsing() {
        let config = NetworkConfig::default();
        let validator = NetworkValidator { config };

        let url = "lit://192.168.1.100/repo.lit";
        let parts = validator.parse_url(url).unwrap();

        assert_eq!(parts.protocol, "lit");
        assert_eq!(parts.host, "192.168.1.100");
        assert_eq!(parts.path, "/repo.lit");
    }
}
