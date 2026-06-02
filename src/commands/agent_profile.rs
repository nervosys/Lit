//! Generic agent profile system — extends the swarm subsystem from SWE-only
//! agents to arbitrary domain agents (CAD designers, EDA engineers, writers,
//! DBAs, reviewers, CI bots, security auditors, etc.).
//!
//! Each agent profile declares capabilities, supported content types, trust
//! domains, and resource limits so Lit can intelligently route work, enforce
//! access policies, and schedule across heterogeneous agent fleets.

use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::AgentProfileResponse;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ── Data types ──────────────────────────────────────────────────────────────

/// Domain classification for agent specialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentDomain {
    /// Software engineering (code, tests, CI/CD)
    Software,
    /// Mechanical / industrial CAD modeling
    Cad,
    /// Electronic design automation (PCB, schematic, FPGA)
    Eda,
    /// Writing — technical docs, manuscripts, legal prose
    Writer,
    /// Database administration — schema, migration, optimization
    Dba,
    /// Code / design review
    Reviewer,
    /// Continuous integration and deployment
    Ci,
    /// Security auditing and compliance
    Security,
    /// Data science and ML pipelines
    DataScience,
    /// DevOps / infrastructure management
    DevOps,
    /// Quality assurance / test automation
    Qa,
    /// Project management / coordination
    ProjectManagement,
    /// General purpose
    General,
    /// Custom domain
    Custom(String),
}

impl std::fmt::Display for AgentDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentDomain::Software => write!(f, "software"),
            AgentDomain::Cad => write!(f, "cad"),
            AgentDomain::Eda => write!(f, "eda"),
            AgentDomain::Writer => write!(f, "writer"),
            AgentDomain::Dba => write!(f, "dba"),
            AgentDomain::Reviewer => write!(f, "reviewer"),
            AgentDomain::Ci => write!(f, "ci"),
            AgentDomain::Security => write!(f, "security"),
            AgentDomain::DataScience => write!(f, "data-science"),
            AgentDomain::DevOps => write!(f, "devops"),
            AgentDomain::Qa => write!(f, "qa"),
            AgentDomain::ProjectManagement => write!(f, "project-management"),
            AgentDomain::General => write!(f, "general"),
            AgentDomain::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// Capabilities an agent can advertise
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    /// Read files / objects
    Read,
    /// Write / modify files
    Write,
    /// Create new branches
    Branch,
    /// Merge branches
    Merge,
    /// Review and approve
    Review,
    /// Deploy / release
    Deploy,
    /// Run tests
    Test,
    /// Run security scans
    SecurityScan,
    /// Generate diffs / patches
    Diff,
    /// Manage large files (LFS operations)
    Lfs,
    /// Create / manage intents
    Intent,
    /// Converge intents to mainline
    Converge,
    /// Manage content type metadata
    ContentMetadata,
    /// Cross-domain coordination (orchestrate other agents)
    Orchestrate,
    /// Structural diff / merge for binary formats
    StructuralAnalysis,
    /// Schema-aware operations (database agents)
    SchemaManagement,
    /// Custom capability
    Custom(String),
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Read => write!(f, "read"),
            Capability::Write => write!(f, "write"),
            Capability::Branch => write!(f, "branch"),
            Capability::Merge => write!(f, "merge"),
            Capability::Review => write!(f, "review"),
            Capability::Deploy => write!(f, "deploy"),
            Capability::Test => write!(f, "test"),
            Capability::SecurityScan => write!(f, "security-scan"),
            Capability::Diff => write!(f, "diff"),
            Capability::Lfs => write!(f, "lfs"),
            Capability::Intent => write!(f, "intent"),
            Capability::Converge => write!(f, "converge"),
            Capability::ContentMetadata => write!(f, "content-metadata"),
            Capability::Orchestrate => write!(f, "orchestrate"),
            Capability::StructuralAnalysis => write!(f, "structural-analysis"),
            Capability::SchemaManagement => write!(f, "schema-management"),
            Capability::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// Trust level assigned to an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// Untrusted — read-only sandbox
    Untrusted = 0,
    /// Limited — can propose changes but not merge
    Limited = 1,
    /// Standard — full read/write within assigned scope
    Standard = 2,
    /// Elevated — can merge, converge, manage intents
    Elevated = 3,
    /// Admin — full control including key management
    Admin = 4,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Untrusted => write!(f, "untrusted"),
            TrustLevel::Limited => write!(f, "limited"),
            TrustLevel::Standard => write!(f, "standard"),
            TrustLevel::Elevated => write!(f, "elevated"),
            TrustLevel::Admin => write!(f, "admin"),
        }
    }
}

/// Resource constraints for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum file size the agent can write (bytes, 0 = unlimited)
    pub max_file_size: u64,
    /// Maximum total storage the agent can consume (bytes, 0 = unlimited)
    pub max_total_storage: u64,
    /// Maximum number of files the agent can modify per commit
    pub max_files_per_commit: u32,
    /// Maximum number of concurrent leases
    pub max_concurrent_leases: u32,
    /// Maximum branch count the agent can own
    pub max_branches: u32,
    /// Rate limit — max operations per minute (0 = unlimited)
    pub max_ops_per_minute: u32,
    /// Whether the agent can access network (fetch/push/clone)
    pub network_access: bool,
    /// Whether the agent can execute hooks/scripts
    pub hook_execution: bool,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_size: 100 * 1024 * 1024, // 100 MB
            max_total_storage: 0,             // unlimited
            max_files_per_commit: 1000,
            max_concurrent_leases: 10,
            max_branches: 5,
            max_ops_per_minute: 60,
            network_access: true,
            hook_execution: false,
        }
    }
}

/// A complete agent profile definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Unique profile identifier
    pub profile_id: String,
    /// Human-readable name
    pub name: String,
    /// Domain specialization
    pub domain: AgentDomain,
    /// Capabilities this agent advertises
    pub capabilities: Vec<Capability>,
    /// Content type IDs this agent can work with (empty = all)
    pub supported_content_types: Vec<String>,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Resource limits
    pub resource_limits: ResourceLimits,
    /// Path patterns this agent is allowed to modify (glob patterns, empty = all)
    pub allowed_paths: Vec<String>,
    /// Path patterns this agent is denied from modifying
    pub denied_paths: Vec<String>,
    /// Description
    pub description: String,
    /// Version of the profile schema
    pub version: String,
    /// Registration timestamp
    pub registered_at: String,
    /// Optional parent profile to inherit from
    pub inherits_from: Option<String>,
    /// Arbitrary metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

// ── Built-in profiles ───────────────────────────────────────────────────────

fn builtin_profiles() -> Vec<AgentProfile> {
    let now = Utc::now().to_rfc3339();
    vec![
        AgentProfile {
            profile_id: "swe-default".into(),
            name: "Software Engineer".into(),
            domain: AgentDomain::Software,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Merge, Capability::Test, Capability::Diff,
                Capability::Intent, Capability::Converge,
            ],
            supported_content_types: vec![],
            trust_level: TrustLevel::Standard,
            resource_limits: ResourceLimits::default(),
            allowed_paths: vec!["**/*.rs".into(), "**/*.py".into(), "**/*.ts".into(), "**/*.js".into(),
                                "**/*.go".into(), "**/*.java".into(), "**/*.c".into(), "**/*.cpp".into(),
                                "**/*.h".into(), "**/*.toml".into(), "**/*.json".into(), "**/*.yaml".into(),
                                "**/*.yml".into(), "**/*.md".into(), "**/*.txt".into()],
            denied_paths: vec![],
            description: "General-purpose software engineering agent".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({}),
        },
        AgentProfile {
            profile_id: "cad-designer".into(),
            name: "CAD Designer".into(),
            domain: AgentDomain::Cad,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Lfs, Capability::Diff, Capability::Intent,
                Capability::StructuralAnalysis, Capability::ContentMetadata,
            ],
            supported_content_types: vec![
                "cad/step".into(), "cad/stl".into(), "cad/iges".into(), "cad/3mf".into(),
            ],
            trust_level: TrustLevel::Standard,
            resource_limits: ResourceLimits {
                max_file_size: 500 * 1024 * 1024, // 500 MB for CAD
                max_files_per_commit: 50,
                max_concurrent_leases: 5,
                ..Default::default()
            },
            allowed_paths: vec!["**/*.step".into(), "**/*.stp".into(), "**/*.stl".into(),
                                "**/*.igs".into(), "**/*.iges".into(), "**/*.3mf".into(),
                                "**/*.obj".into(), "**/*.dxf".into()],
            denied_paths: vec!["src/**".into()],
            description: "Mechanical/industrial CAD modeling agent with LFS and structural diff support".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({"tools": ["openscad", "freecad", "solidworks"]}),
        },
        AgentProfile {
            profile_id: "eda-engineer".into(),
            name: "EDA Engineer".into(),
            domain: AgentDomain::Eda,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Diff, Capability::Intent, Capability::StructuralAnalysis,
                Capability::ContentMetadata,
            ],
            supported_content_types: vec![
                "eda/kicad-pcb".into(), "eda/kicad-sch".into(), "eda/gerber".into(), "eda/spice".into(),
            ],
            trust_level: TrustLevel::Standard,
            resource_limits: ResourceLimits {
                max_files_per_commit: 100,
                max_concurrent_leases: 8,
                ..Default::default()
            },
            allowed_paths: vec!["**/*.kicad_pcb".into(), "**/*.kicad_sch".into(),
                                "**/*.gbr".into(), "**/*.ger".into(), "**/*.spice".into(),
                                "**/*.lib".into(), "**/*.bom".into()],
            denied_paths: vec![],
            description: "Electronic design automation agent for PCB/schematic/FPGA work".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({"tools": ["kicad", "ltspice", "verilator"]}),
        },
        AgentProfile {
            profile_id: "tech-writer".into(),
            name: "Technical Writer".into(),
            domain: AgentDomain::Writer,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Diff, Capability::Intent, Capability::ContentMetadata,
            ],
            supported_content_types: vec![
                "manuscript/latex".into(), "manuscript/docx".into(),
                "manuscript/typst".into(), "manuscript/asciidoc".into(),
            ],
            trust_level: TrustLevel::Standard,
            resource_limits: ResourceLimits {
                max_file_size: 50 * 1024 * 1024,
                max_files_per_commit: 200,
                ..Default::default()
            },
            allowed_paths: vec!["**/*.md".into(), "**/*.tex".into(), "**/*.typ".into(),
                                "**/*.adoc".into(), "**/*.rst".into(), "**/*.docx".into(),
                                "**/*.txt".into(), "docs/**".into()],
            denied_paths: vec!["src/**".into()],
            description: "Technical writing agent for documentation, manuscripts, and publications".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({"languages": ["en", "de", "fr", "ja"]}),
        },
        AgentProfile {
            profile_id: "dba".into(),
            name: "Database Administrator".into(),
            domain: AgentDomain::Dba,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Diff, Capability::Intent, Capability::SchemaManagement,
                Capability::ContentMetadata,
            ],
            supported_content_types: vec![
                "db/sqlite".into(), "db/csv".into(), "db/parquet".into(), "db/sql-migration".into(),
            ],
            trust_level: TrustLevel::Elevated,
            resource_limits: ResourceLimits {
                max_file_size: 1024 * 1024 * 1024, // 1 GB for databases
                max_files_per_commit: 50,
                ..Default::default()
            },
            allowed_paths: vec!["**/*.sql".into(), "**/*.sqlite".into(), "**/*.db".into(),
                                "**/*.csv".into(), "**/*.parquet".into(), "migrations/**".into()],
            denied_paths: vec![],
            description: "Database administration agent for schema, migration, and data versioning".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({"tools": ["sqlite", "postgres", "duckdb"]}),
        },
        AgentProfile {
            profile_id: "reviewer".into(),
            name: "Code & Design Reviewer".into(),
            domain: AgentDomain::Reviewer,
            capabilities: vec![
                Capability::Read, Capability::Review, Capability::Diff,
                Capability::Converge, Capability::ContentMetadata,
            ],
            supported_content_types: vec![], // reviews all types
            trust_level: TrustLevel::Elevated,
            resource_limits: ResourceLimits {
                max_files_per_commit: 0,
                max_file_size: 0,
                network_access: false,
                ..Default::default()
            },
            allowed_paths: vec![],
            denied_paths: vec![],
            description: "Read-only review agent that can approve and converge but not modify files".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({}),
        },
        AgentProfile {
            profile_id: "ci-bot".into(),
            name: "CI/CD Bot".into(),
            domain: AgentDomain::Ci,
            capabilities: vec![
                Capability::Read, Capability::Test, Capability::Deploy,
                Capability::SecurityScan, Capability::Diff,
            ],
            supported_content_types: vec![],
            trust_level: TrustLevel::Elevated,
            resource_limits: ResourceLimits {
                network_access: true,
                hook_execution: true,
                ..Default::default()
            },
            allowed_paths: vec![],
            denied_paths: vec![],
            description: "CI/CD automation agent for build, test, and deploy pipelines".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({}),
        },
        AgentProfile {
            profile_id: "security-auditor".into(),
            name: "Security Auditor".into(),
            domain: AgentDomain::Security,
            capabilities: vec![
                Capability::Read, Capability::Review, Capability::SecurityScan,
                Capability::Diff,
            ],
            supported_content_types: vec![],
            trust_level: TrustLevel::Elevated,
            resource_limits: ResourceLimits {
                max_file_size: 0,
                max_files_per_commit: 0,
                network_access: false,
                hook_execution: false,
                ..Default::default()
            },
            allowed_paths: vec![],
            denied_paths: vec![],
            description: "Security auditing agent — read-only scanning and compliance verification".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({}),
        },
        AgentProfile {
            profile_id: "data-scientist".into(),
            name: "Data Scientist".into(),
            domain: AgentDomain::DataScience,
            capabilities: vec![
                Capability::Read, Capability::Write, Capability::Branch,
                Capability::Lfs, Capability::Intent, Capability::ContentMetadata,
                Capability::SchemaManagement,
            ],
            supported_content_types: vec![
                "scientific/hdf5".into(), "scientific/jupyter".into(),
                "db/parquet".into(), "db/csv".into(),
            ],
            trust_level: TrustLevel::Standard,
            resource_limits: ResourceLimits {
                max_file_size: 2 * 1024 * 1024 * 1024, // 2 GB for datasets
                max_concurrent_leases: 3,
                ..Default::default()
            },
            allowed_paths: vec!["**/*.ipynb".into(), "**/*.h5".into(), "**/*.hdf5".into(),
                                "**/*.parquet".into(), "**/*.csv".into(), "**/*.py".into(),
                                "data/**".into(), "notebooks/**".into(), "models/**".into()],
            denied_paths: vec![],
            description: "Data science agent for notebooks, datasets, and ML pipeline versioning".into(),
            version: "1.0".into(),
            registered_at: now.clone(),
            inherits_from: None,
            metadata: serde_json::json!({"frameworks": ["pytorch", "tensorflow", "scikit-learn"]}),
        },
        AgentProfile {
            profile_id: "orchestrator".into(),
            name: "Multi-Agent Orchestrator".into(),
            domain: AgentDomain::General,
            capabilities: vec![
                Capability::Read, Capability::Orchestrate, Capability::Intent,
                Capability::Converge, Capability::Review,
            ],
            supported_content_types: vec![],
            trust_level: TrustLevel::Admin,
            resource_limits: ResourceLimits {
                max_files_per_commit: 0,
                max_file_size: 0,
                network_access: true,
                ..Default::default()
            },
            allowed_paths: vec![],
            denied_paths: vec![],
            description: "Meta-agent that orchestrates, coordinates, and routes work across domain-specific agents".into(),
            version: "1.0".into(),
            registered_at: now,
            inherits_from: None,
            metadata: serde_json::json!({}),
        },
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn profiles_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("agent-profiles")
}

fn save_profile(repo_root: &Path, profile: &AgentProfile) -> Result<(), LitError> {
    let dir = profiles_dir(repo_root);
    fs::create_dir_all(&dir).map_err(|e| LitError::io(e.to_string()))?;
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| LitError::general(format!("Serialize profile: {}", e)))?;
    fs::write(dir.join(format!("{}.json", profile.profile_id)), json)
        .map_err(|e| LitError::io(e.to_string()))?;
    Ok(())
}

fn load_all_profiles(repo_root: &Path) -> Result<Vec<AgentProfile>, LitError> {
    let dir = profiles_dir(repo_root);
    let mut profiles = builtin_profiles();

    if dir.exists() {
        for entry in fs::read_dir(&dir).map_err(|e| LitError::io(e.to_string()))? {
            let entry = entry.map_err(|e| LitError::io(e.to_string()))?;
            if entry
                .path()
                .extension()
                .map(|e| e == "json")
                .unwrap_or(false)
            {
                let json =
                    fs::read_to_string(entry.path()).map_err(|e| LitError::io(e.to_string()))?;
                if let Ok(p) = serde_json::from_str::<AgentProfile>(&json) {
                    profiles.retain(|b| b.profile_id != p.profile_id);
                    profiles.push(p);
                }
            }
        }
    }
    Ok(profiles)
}

// ── Public API ──────────────────────────────────────────────────────────────

/// List all agent profiles, optionally filtered by domain
pub fn execute_list(domain_filter: Option<String>) -> Result<AgentProfileResponse, LitError> {
    let repo_root = find_repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut profiles = load_all_profiles(&repo_root)?;

    if let Some(ref domain) = domain_filter {
        profiles.retain(|p| p.domain.to_string() == *domain);
    }

    let count = profiles.len();
    let summary: Vec<serde_json::Value> = profiles
        .iter()
        .map(|p| {
            serde_json::json!({
                "profile_id": p.profile_id,
                "name": p.name,
                "domain": p.domain.to_string(),
                "trust_level": p.trust_level.to_string(),
                "capabilities": p.capabilities.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                "content_types": p.supported_content_types,
            })
        })
        .collect();

    Ok(AgentProfileResponse {
        action: "list".into(),
        profile_id: None,
        message: format!("{} agent profile(s)", count),
        details: Some(serde_json::to_value(&summary).unwrap_or_default()),
    })
}

/// Show a specific agent profile
pub fn execute_show(profile_id: String) -> Result<AgentProfileResponse, LitError> {
    let repo_root = find_repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let profiles = load_all_profiles(&repo_root)?;

    let profile = profiles
        .iter()
        .find(|p| p.profile_id == profile_id)
        .ok_or_else(|| LitError::general(format!("Agent profile not found: {}", profile_id)))?;

    Ok(AgentProfileResponse {
        action: "show".into(),
        profile_id: Some(profile.profile_id.clone()),
        message: format!(
            "{} ({}, trust={})",
            profile.name, profile.domain, profile.trust_level
        ),
        details: Some(serde_json::to_value(profile).unwrap_or_default()),
    })
}

/// Register a custom agent profile
#[allow(clippy::too_many_arguments)]
pub fn execute_register(
    profile_id: String,
    name: String,
    domain: String,
    capabilities: Vec<String>,
    trust_level: Option<String>,
    content_types: Vec<String>,
    allowed_paths: Vec<String>,
    denied_paths: Vec<String>,
) -> Result<AgentProfileResponse, LitError> {
    let repo_root = find_repo_root()?;

    let domain_enum = match domain.as_str() {
        "software" => AgentDomain::Software,
        "cad" => AgentDomain::Cad,
        "eda" => AgentDomain::Eda,
        "writer" => AgentDomain::Writer,
        "dba" => AgentDomain::Dba,
        "reviewer" => AgentDomain::Reviewer,
        "ci" => AgentDomain::Ci,
        "security" => AgentDomain::Security,
        "data-science" => AgentDomain::DataScience,
        "devops" => AgentDomain::DevOps,
        "qa" => AgentDomain::Qa,
        "project-management" => AgentDomain::ProjectManagement,
        "general" => AgentDomain::General,
        other => AgentDomain::Custom(other.to_string()),
    };

    let caps: Vec<Capability> = capabilities
        .iter()
        .map(|c| match c.as_str() {
            "read" => Capability::Read,
            "write" => Capability::Write,
            "branch" => Capability::Branch,
            "merge" => Capability::Merge,
            "review" => Capability::Review,
            "deploy" => Capability::Deploy,
            "test" => Capability::Test,
            "security-scan" => Capability::SecurityScan,
            "diff" => Capability::Diff,
            "lfs" => Capability::Lfs,
            "intent" => Capability::Intent,
            "converge" => Capability::Converge,
            "content-metadata" => Capability::ContentMetadata,
            "orchestrate" => Capability::Orchestrate,
            "structural-analysis" => Capability::StructuralAnalysis,
            "schema-management" => Capability::SchemaManagement,
            other => Capability::Custom(other.to_string()),
        })
        .collect();

    let trust = match trust_level.as_deref() {
        Some("untrusted") => TrustLevel::Untrusted,
        Some("limited") => TrustLevel::Limited,
        Some("elevated") => TrustLevel::Elevated,
        Some("admin") => TrustLevel::Admin,
        _ => TrustLevel::Standard,
    };

    let profile = AgentProfile {
        profile_id: profile_id.clone(),
        name: name.clone(),
        domain: domain_enum,
        capabilities: caps,
        supported_content_types: content_types,
        trust_level: trust,
        resource_limits: ResourceLimits::default(),
        allowed_paths,
        denied_paths,
        description: format!("Custom agent profile: {}", name),
        version: "1.0".into(),
        registered_at: Utc::now().to_rfc3339(),
        inherits_from: None,
        metadata: serde_json::json!({}),
    };

    save_profile(&repo_root, &profile)?;

    Ok(AgentProfileResponse {
        action: "register".into(),
        profile_id: Some(profile_id),
        message: format!("Agent profile '{}' registered", name),
        details: Some(serde_json::to_value(&profile).unwrap_or_default()),
    })
}

/// List capabilities available for a given domain
pub fn execute_capabilities(domain: Option<String>) -> Result<AgentProfileResponse, LitError> {
    let repo_root = find_repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let profiles = load_all_profiles(&repo_root)?;

    let filtered: Vec<&AgentProfile> = if let Some(ref d) = domain {
        profiles
            .iter()
            .filter(|p| p.domain.to_string() == *d)
            .collect()
    } else {
        profiles.iter().collect()
    };

    // Aggregate unique capabilities
    let mut all_caps: Vec<String> = filtered
        .iter()
        .flat_map(|p| p.capabilities.iter().map(|c| c.to_string()))
        .collect();
    all_caps.sort();
    all_caps.dedup();

    // Aggregate unique domains
    let mut all_domains: Vec<String> = profiles.iter().map(|p| p.domain.to_string()).collect();
    all_domains.sort();
    all_domains.dedup();

    Ok(AgentProfileResponse {
        action: "capabilities".into(),
        profile_id: None,
        message: format!(
            "{} unique capability/ies across {} domain(s)",
            all_caps.len(),
            all_domains.len()
        ),
        details: Some(serde_json::json!({
            "capabilities": all_caps,
            "domains": all_domains,
            "profiles_by_domain": all_domains.iter().map(|d| {
                let count = profiles.iter().filter(|p| p.domain.to_string() == *d).count();
                serde_json::json!({"domain": d, "profile_count": count})
            }).collect::<Vec<_>>(),
        })),
    })
}

/// Remove a custom agent profile
pub fn execute_remove(profile_id: String) -> Result<AgentProfileResponse, LitError> {
    let repo_root = find_repo_root()?;
    let dir = profiles_dir(&repo_root);
    let path = dir.join(format!("{}.json", profile_id));

    if !path.exists() {
        // Could be a builtin — can't remove builtins
        let builtins = builtin_profiles();
        if builtins.iter().any(|p| p.profile_id == profile_id) {
            return Err(LitError::general(format!(
                "Cannot remove built-in profile: {}",
                profile_id
            )));
        }
        return Err(LitError::general(format!(
            "Agent profile not found: {}",
            profile_id
        )));
    }

    fs::remove_file(&path).map_err(|e| LitError::io(e.to_string()))?;

    Ok(AgentProfileResponse {
        action: "remove".into(),
        profile_id: Some(profile_id.clone()),
        message: format!("Agent profile '{}' removed", profile_id),
        details: None,
    })
}
