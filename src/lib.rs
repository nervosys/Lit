//! # Lit — The Agentic-First Distributed Version Control System
//!
//! Lit is a complete Git replacement written in Rust, designed for AI agents first and humans
//! second. Every command emits structured JSON by default with typed error codes, merge conflicts
//! as structured objects, and native MCP/REST API integration.
//!
//! ## Key Features
//!
//! - **Structured output**: All commands return typed JSON responses via [`response`] types
//! - **Post-quantum cryptography**: SHA3-512 + BLAKE3 hashing, ML-DSA signatures, AES-256-GCM encryption
//! - **MCP server**: JSON-RPC 2.0 tool server with 30 tools via [`commands::mcp_serve`]
//! - **Machine-readable ontology**: Full command/type discovery via [`ontology`]
//! - **Typed errors**: [`errors::LitError`] with error codes, user messages, and remediation hints
//!
//! ## Usage
//!
//! ```no_run
//! use lit::commands;
//! use lit::ontology;
//!
//! // Generate the full JSON Schema for agent SDK integration
//! let schema = ontology::generate_schemas();
//!
//! // Get the machine-readable ontology
//! let ont = ontology::get_ontology();
//! ```

/// VCS command implementations (init, add, commit, status, diff, merge, etc.)
pub mod api;
pub mod commands;
/// Hierarchical configuration (repo-local → user global → env vars)
pub mod config;
/// Core VCS primitives: objects (blob, tree, commit, tag), refs, and repo discovery
pub mod core;
/// Post-quantum cryptographic operations: hashing, signing, encryption, key management
pub mod crypto;
/// Typed error system with machine-readable error codes and remediation hints
pub mod errors;
pub mod events;
pub mod federation;
/// Output formatting (JSON, human-readable, MessagePack)
pub mod formatter;
pub mod identity;
/// Network transport layer: HTTPS, SSH, lit:// protocol, air-gap enforcement
pub mod network;
/// Machine-readable ontology and JSON Schema generation for agent discovery
pub mod ontology;
/// Typed response structs for all 42+ CLI commands
pub mod response;
/// Object store and index (staging area) persistence
pub mod storage;

// Re-export CLI types that are used by commands
#[derive(Clone, Debug)]
pub enum RemoteCommands {
    Add { name: String, url: String },
    Remove { name: String },
    List { verbose: bool },
}

#[derive(Clone, Debug)]
pub enum ConfigCommands {
    Show,
    Get { key: String },
    Set { key: String, value: String },
}

#[derive(Clone, Debug)]
pub enum StashCommands {
    Push { message: Option<String> },
    Pop,
    Apply { index: Option<usize> },
    List,
    Drop { index: Option<usize> },
}

#[derive(Clone, Debug)]
pub enum BisectCommands {
    Start,
    Good { commit: String },
    Bad { commit: String },
    Reset,
}

#[derive(Clone, Debug)]
pub enum TxCommands {
    Begin,
    Commit,
    Rollback,
}

#[derive(Clone, Debug)]
pub enum SwarmCommands {
    Register {
        agent_id: String,
    },
    List,
    LeaseAcquire {
        agent_id: String,
        path: String,
        duration: u64,
    },
    LeaseRelease {
        agent_id: String,
        path: String,
    },
    LeaseList,
}

#[derive(Clone, Debug)]
pub enum LfsCommands {
    Track { patterns: Vec<String> },
    Migrate { threshold: Option<u64> },
}
