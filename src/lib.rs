// Lit library - expose modules for integration testing and library use
pub mod commands;
pub mod config;
pub mod formatter;
pub mod core;
pub mod crypto;
pub mod errors;
pub mod network;
pub mod ontology;
pub mod response;
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