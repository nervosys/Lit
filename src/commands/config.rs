use crate::network::{AirgapConfig, NetworkConfig};
use crate::response::{ConfigEntry, ConfigResponse};

pub fn execute(command: Option<crate::ConfigCommands>) -> Result<ConfigResponse, String> {
    match command {
        Some(crate::ConfigCommands::Show) | None => show_config(),
        Some(crate::ConfigCommands::Get { key }) => get_config(&key),
        Some(crate::ConfigCommands::Set { key, value }) => set_config(&key, &value),
    }
}

fn show_config() -> Result<ConfigResponse, String> {
    let network_config = NetworkConfig::load()?;
    let airgap_config = AirgapConfig::load()?;

    let mut entries = Vec::new();

    entries.push(ConfigEntry {
        key: "airgap.enabled".to_string(),
        value: airgap_config.enabled.to_string(),
    });
    entries.push(ConfigEntry {
        key: "airgap.strict_mode".to_string(),
        value: airgap_config.strict_mode.to_string(),
    });
    entries.push(ConfigEntry {
        key: "airgap.allowed_transports".to_string(),
        value: format!("{:?}", airgap_config.allowed_transports),
    });
    if !airgap_config.allowed_media.is_empty() {
        entries.push(ConfigEntry {
            key: "airgap.allowed_media".to_string(),
            value: airgap_config.allowed_media.join(", "),
        });
    }
    if !airgap_config.allowed_shares.is_empty() {
        entries.push(ConfigEntry {
            key: "airgap.allowed_shares".to_string(),
            value: airgap_config.allowed_shares.join(", "),
        });
    }
    entries.push(ConfigEntry {
        key: "network.allowed_networks".to_string(),
        value: network_config.allowed_networks.join(", "),
    });
    entries.push(ConfigEntry {
        key: "network.allowed_hosts".to_string(),
        value: if network_config.allowed_hosts.is_empty() {
            "(none)".to_string()
        } else {
            network_config.allowed_hosts.join(", ")
        },
    });
    entries.push(ConfigEntry {
        key: "security.network_audit_log".to_string(),
        value: network_config.audit_log.to_string(),
    });
    if let Some(path) = &network_config.audit_log_path {
        entries.push(ConfigEntry {
            key: "security.network_audit_log_path".to_string(),
            value: path.clone(),
        });
    }
    entries.push(ConfigEntry {
        key: "security.airgap_audit_log".to_string(),
        value: airgap_config.audit_log.to_string(),
    });
    if let Some(path) = &airgap_config.audit_log_path {
        entries.push(ConfigEntry {
            key: "security.airgap_audit_log_path".to_string(),
            value: path.clone(),
        });
    }

    Ok(ConfigResponse::Show { entries })
}

fn get_config(key: &str) -> Result<ConfigResponse, String> {
    let network_config = NetworkConfig::load()?;
    let airgap_config = AirgapConfig::load()?;

    let value = match key {
        "airgap.enabled" => airgap_config.enabled.to_string(),
        "airgap.strict_mode" => airgap_config.strict_mode.to_string(),
        "network.allowed_networks" => network_config.allowed_networks.join(", "),
        "network.allowed_hosts" => network_config.allowed_hosts.join(", "),
        "security.audit_log" => network_config.audit_log.to_string(),
        "security.audit_log_path" => network_config
            .audit_log_path
            .unwrap_or_default(),
        _ => return Err(format!("Unknown configuration key: {}", key)),
    };

    Ok(ConfigResponse::Get {
        key: key.to_string(),
        value,
    })
}

fn set_config(key: &str, value: &str) -> Result<ConfigResponse, String> {
    match key {
        "airgap.enabled" => {
            let mut config = AirgapConfig::load()?;
            config.enabled = value
                .parse::<bool>()
                .map_err(|_| "Invalid boolean value (use 'true' or 'false')".to_string())?;
            config.save()?;
            Ok(ConfigResponse::Set {
                key: key.to_string(),
                value: value.to_string(),
            })
        }
        "airgap.strict_mode" => {
            let mut config = AirgapConfig::load()?;
            config.strict_mode = value
                .parse::<bool>()
                .map_err(|_| "Invalid boolean value (use 'true' or 'false')".to_string())?;
            config.save()?;
            Ok(ConfigResponse::Set {
                key: key.to_string(),
                value: value.to_string(),
            })
        }
        _ => Err(format!(
            "Setting '{}' is not supported via command line.\n\
                 Supported keys: airgap.enabled, airgap.strict_mode\n\
                 For other settings, edit ~/.lit/airgap.toml or ~/.litconfig directly.",
            key
        )),
    }
}
