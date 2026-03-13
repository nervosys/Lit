use serde::{Deserialize, Serialize};
use std::path::Path;

/// Unified configuration with repo-local, user-global, and system hierarchy.
/// Priority: CLI args > env vars > repo-local > user-global > defaults
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LitConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub merge: MergeConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub lfs: LfsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub default_branch: String,
    pub default_output: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        CoreConfig {
            default_branch: "main".to_string(),
            default_output: "json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    pub auto_sign: bool,
    pub default_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    pub default_strategy: String,
    pub auto_resolve: bool,
}

impl Default for MergeConfig {
    fn default() -> Self {
        MergeConfig {
            default_strategy: "recursive".to_string(),
            auto_resolve: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub encryption: String,
    pub fips_mode: bool,
    pub audit_log: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            encryption: "aes-256-gcm".to_string(),
            fips_mode: false,
            audit_log: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Use parallel I/O for status, hashing, pack operations
    pub parallel_io: bool,
    /// Number of threads (0 = auto-detect)
    pub threads: usize,
    /// Pack objects when loose object count exceeds this threshold
    pub auto_pack_threshold: usize,
    /// Large file threshold in bytes for LFS (default: 10 MB)
    pub lfs_threshold: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        PerformanceConfig {
            parallel_io: true,
            threads: 0,
            auto_pack_threshold: 1000,
            lfs_threshold: 10 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LfsConfig {
    /// Enable large file storage
    pub enabled: bool,
    /// Glob patterns for files to track as LFS
    pub track_patterns: Vec<String>,
}

impl LitConfig {
    /// Load configuration with full hierarchy:
    /// repo-local .lit/config.toml > user ~/.litconfig.toml > defaults
    pub fn load(repo_path: Option<&Path>) -> Self {
        let mut config = LitConfig::default();

        // Layer 1: User global config
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".litconfig.toml");
            if let Ok(content) = std::fs::read_to_string(&global_path) {
                if let Ok(global) = toml::from_str::<LitConfig>(&content) {
                    config = config.merge_with(global);
                }
            }
        }

        // Layer 2: Repo-local config (overrides global)
        if let Some(repo) = repo_path {
            let local_path = repo.join(".lit").join("config.toml");
            if let Ok(content) = std::fs::read_to_string(&local_path) {
                if let Ok(local) = toml::from_str::<LitConfig>(&content) {
                    config = config.merge_with(local);
                }
            }
        }

        // Layer 3: Environment variables (override everything)
        if let Ok(v) = std::env::var("LIT_DEFAULT_BRANCH") {
            config.core.default_branch = v;
        }
        if let Ok(v) = std::env::var("LIT_OUTPUT") {
            config.core.default_output = v;
        }
        if let Ok(v) = std::env::var("LIT_FIPS_MODE") {
            config.security.fips_mode = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("LIT_PARALLEL") {
            config.performance.parallel_io = v != "false" && v != "0";
        }

        config
    }

    /// Save repo-local configuration
    pub fn save_local(&self, repo_path: &Path) -> Result<(), String> {
        let config_path = repo_path.join(".lit").join("config.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&config_path, content).map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Save user-global configuration
    pub fn save_global(&self) -> Result<(), String> {
        let home = dirs::home_dir().ok_or("Could not determine home directory")?;
        let config_path = home.join(".litconfig.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&config_path, content).map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Get a config value by dotted key path
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "core.default_branch" => Some(self.core.default_branch.clone()),
            "core.default_output" => Some(self.core.default_output.clone()),
            "agent.auto_sign" => Some(self.agent.auto_sign.to_string()),
            "merge.default_strategy" => Some(self.merge.default_strategy.clone()),
            "merge.auto_resolve" => Some(self.merge.auto_resolve.to_string()),
            "security.encryption" => Some(self.security.encryption.clone()),
            "security.fips_mode" => Some(self.security.fips_mode.to_string()),
            "security.audit_log" => Some(self.security.audit_log.to_string()),
            "performance.parallel_io" => Some(self.performance.parallel_io.to_string()),
            "performance.threads" => Some(self.performance.threads.to_string()),
            "performance.auto_pack_threshold" => {
                Some(self.performance.auto_pack_threshold.to_string())
            }
            "performance.lfs_threshold" => Some(self.performance.lfs_threshold.to_string()),
            "lfs.enabled" => Some(self.lfs.enabled.to_string()),
            _ => None,
        }
    }

    /// Set a config value by dotted key path
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "core.default_branch" => self.core.default_branch = value.to_string(),
            "core.default_output" => self.core.default_output = value.to_string(),
            "agent.auto_sign" => {
                self.agent.auto_sign = value == "true" || value == "1";
            }
            "merge.default_strategy" => self.merge.default_strategy = value.to_string(),
            "merge.auto_resolve" => {
                self.merge.auto_resolve = value == "true" || value == "1";
            }
            "security.encryption" => self.security.encryption = value.to_string(),
            "security.fips_mode" => {
                self.security.fips_mode = value == "true" || value == "1";
            }
            "security.audit_log" => {
                self.security.audit_log = value == "true" || value == "1";
            }
            "performance.parallel_io" => {
                self.performance.parallel_io = value == "true" || value == "1";
            }
            "performance.threads" => {
                self.performance.threads = value
                    .parse()
                    .map_err(|_| format!("Invalid thread count: {}", value))?;
            }
            "performance.auto_pack_threshold" => {
                self.performance.auto_pack_threshold = value
                    .parse()
                    .map_err(|_| format!("Invalid threshold: {}", value))?;
            }
            "performance.lfs_threshold" => {
                self.performance.lfs_threshold = value
                    .parse()
                    .map_err(|_| format!("Invalid threshold: {}", value))?;
            }
            "lfs.enabled" => {
                self.lfs.enabled = value == "true" || value == "1";
            }
            _ => return Err(format!("Unknown config key: {}", key)),
        }
        Ok(())
    }

    /// Get all config entries as key-value pairs
    pub fn entries(&self) -> Vec<(String, String)> {
        vec![
            (
                "core.default_branch".into(),
                self.core.default_branch.clone(),
            ),
            (
                "core.default_output".into(),
                self.core.default_output.clone(),
            ),
            ("agent.auto_sign".into(), self.agent.auto_sign.to_string()),
            (
                "merge.default_strategy".into(),
                self.merge.default_strategy.clone(),
            ),
            (
                "merge.auto_resolve".into(),
                self.merge.auto_resolve.to_string(),
            ),
            (
                "security.encryption".into(),
                self.security.encryption.clone(),
            ),
            (
                "security.fips_mode".into(),
                self.security.fips_mode.to_string(),
            ),
            (
                "security.audit_log".into(),
                self.security.audit_log.to_string(),
            ),
            (
                "performance.parallel_io".into(),
                self.performance.parallel_io.to_string(),
            ),
            (
                "performance.threads".into(),
                self.performance.threads.to_string(),
            ),
            (
                "performance.auto_pack_threshold".into(),
                self.performance.auto_pack_threshold.to_string(),
            ),
            (
                "performance.lfs_threshold".into(),
                self.performance.lfs_threshold.to_string(),
            ),
            ("lfs.enabled".into(), self.lfs.enabled.to_string()),
        ]
    }

    /// Merge another config on top of this one (other wins on conflicts)
    fn merge_with(mut self, other: LitConfig) -> Self {
        // Only override non-default values
        if other.core.default_branch != CoreConfig::default().default_branch {
            self.core.default_branch = other.core.default_branch;
        }
        if other.core.default_output != CoreConfig::default().default_output {
            self.core.default_output = other.core.default_output;
        }
        if other.agent.auto_sign {
            self.agent.auto_sign = true;
        }
        if other.agent.default_metadata.is_some() {
            self.agent.default_metadata = other.agent.default_metadata;
        }
        if other.merge.default_strategy != MergeConfig::default().default_strategy {
            self.merge.default_strategy = other.merge.default_strategy;
        }
        if other.merge.auto_resolve {
            self.merge.auto_resolve = true;
        }
        if other.security.fips_mode {
            self.security.fips_mode = true;
        }
        if !other.performance.parallel_io {
            self.performance.parallel_io = false;
        }
        if other.performance.threads != 0 {
            self.performance.threads = other.performance.threads;
        }
        if other.lfs.enabled {
            self.lfs.enabled = true;
        }
        if !other.lfs.track_patterns.is_empty() {
            self.lfs.track_patterns = other.lfs.track_patterns;
        }
        self
    }
}
