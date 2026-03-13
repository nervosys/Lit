use crate::core::find_repo_root;
use crate::network::{AirgapConfig, AirgapValidator, NetworkValidator};
use crate::response::{RemoteEntry, RemoteResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Remote {
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteConfig {
    remotes: HashMap<String, Remote>,
}

impl RemoteConfig {
    fn load(repo_path: &std::path::Path) -> Result<Self, String> {
        let config_path = repo_path.join(".lit").join("remotes");

        if !config_path.exists() {
            return Ok(RemoteConfig {
                remotes: HashMap::new(),
            });
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read remotes config: {}", e))?;

        serde_json::from_str(&content).map_err(|e| format!("Failed to parse remotes config: {}", e))
    }

    fn save(&self, repo_path: &std::path::Path) -> Result<(), String> {
        let config_path = repo_path.join(".lit").join("remotes");

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize remotes config: {}", e))?;

        fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write remotes config: {}", e))
    }
}

pub fn execute(command: Option<crate::RemoteCommands>) -> Result<RemoteResponse, String> {
    let repo_root = find_repo_root()?;

    match command {
        Some(crate::RemoteCommands::Add { name, url }) => {
            // Check if airgap mode is enabled
            if AirgapConfig::is_airgap_mode() {
                let validator = AirgapValidator::new()?;
                validator.validate_transport(&url)?;
            } else if url.starts_with("lit://") {
                let validator = NetworkValidator::new()?;
                validator.validate_url(&url)?;
            }

            let mut config = RemoteConfig::load(&repo_root)?;
            if config.remotes.contains_key(&name) {
                return Err(format!("Remote '{}' already exists", name));
            }
            config.remotes.insert(
                name.clone(),
                Remote { url: url.clone() },
            );
            config.save(&repo_root)?;
            Ok(RemoteResponse::Add { name, url })
        }
        Some(crate::RemoteCommands::Remove { name }) => {
            let mut config = RemoteConfig::load(&repo_root)?;
            if config.remotes.remove(&name).is_none() {
                return Err(format!("Remote '{}' not found", name));
            }
            config.save(&repo_root)?;
            Ok(RemoteResponse::Remove { name })
        }
        Some(crate::RemoteCommands::List { verbose: _ }) | None => {
            let config = RemoteConfig::load(&repo_root)?;
            let remotes = config
                .remotes
                .into_iter()
                .map(|(name, remote)| RemoteEntry {
                    name,
                    url: remote.url,
                })
                .collect();
            Ok(RemoteResponse::List { remotes })
        }
    }
}
