//! Datacenter deployment optimizations — cluster node management, object store
//! sharding, replication factor control, health monitoring, Prometheus-style
//! metrics, and connection pooling configuration.
//!
//! These features enable Lit to operate in distributed datacenter environments
//! with high availability, fault tolerance, and observability requirements.

use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::DatacenterResponse;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Data types ──────────────────────────────────────────────────────────────

/// Role a node plays in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRole {
    /// Full read-write primary
    Primary,
    /// Read replica
    Replica,
    /// Relay / edge cache only
    Relay,
    /// Metrics & monitoring observer
    Observer,
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeRole::Primary => write!(f, "primary"),
            NodeRole::Replica => write!(f, "replica"),
            NodeRole::Relay => write!(f, "relay"),
            NodeRole::Observer => write!(f, "observer"),
        }
    }
}

/// Health status of a node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unreachable,
    Draining,
    Bootstrapping,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unreachable => write!(f, "unreachable"),
            HealthStatus::Draining => write!(f, "draining"),
            HealthStatus::Bootstrapping => write!(f, "bootstrapping"),
        }
    }
}

/// Sharding strategy for distributing objects across nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShardStrategy {
    /// Consistent hash ring (default)
    ConsistentHash,
    /// Range-based on object hash prefix
    RangePrefix,
    /// Round-robin object distribution
    RoundRobin,
    /// Domain-aware — keep a content domain's objects co-located
    DomainAffinity,
}

/// Replication mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReplicationMode {
    /// Synchronous — wait for all replicas to confirm
    Synchronous,
    /// Async — primary confirms immediately, replicas catch up
    Asynchronous,
    /// Semi-sync — wait for at least quorum to confirm
    SemiSync,
}

/// A registered datacenter cluster node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node identifier
    pub node_id: String,
    /// Display name
    pub name: String,
    /// Network endpoint (host:port or URL)
    pub endpoint: String,
    /// Region / availability zone
    pub region: String,
    /// Node role
    pub role: NodeRole,
    /// Current health
    pub health: HealthStatus,
    /// Shard assignments (hex prefixes this node owns)
    pub shard_ranges: Vec<String>,
    /// Last heartbeat timestamp
    pub last_heartbeat: String,
    /// Node capacity metrics
    pub capacity: NodeCapacity,
    /// ISO 8601 date the node was registered
    pub registered_at: String,
}

/// Capacity and utilization metrics for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    /// Total storage in bytes
    pub storage_total: u64,
    /// Used storage in bytes
    pub storage_used: u64,
    /// Object count on this node
    pub object_count: u64,
    /// Maximum concurrent connections
    pub max_connections: u32,
    /// Current active connections
    pub active_connections: u32,
    /// CPU utilization (0.0 – 1.0)
    pub cpu_utilization: f64,
    /// Memory utilization (0.0 – 1.0)
    pub memory_utilization: f64,
}

/// Cluster-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Replication factor (how many copies per object)
    pub replication_factor: u32,
    /// Replication mode
    pub replication_mode: ReplicationMode,
    /// Sharding strategy
    pub shard_strategy: ShardStrategy,
    /// Number of virtual shards (powers of 16, e.g. 256 = 2 hex chars)
    pub shard_count: u32,
    /// Connection pool size per node
    pub connection_pool_size: u32,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u32,
    /// Node timeout before marking unreachable
    pub node_timeout_secs: u32,
    /// Whether to enable Prometheus-style metrics endpoint
    pub metrics_enabled: bool,
    /// Metrics endpoint port (default: 9090)
    pub metrics_port: u16,
    /// Maximum object size before chunked transfer (bytes)
    pub chunk_threshold: u64,
    /// Chunk size for large object transfer (bytes)
    pub chunk_size: u64,
    /// Compression for inter-node transfer
    pub transfer_compression: bool,
    /// Enable read replicas for load balancing reads
    pub read_load_balance: bool,
    /// Write concern — how many nodes must confirm a write
    pub write_concern: u32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            replication_mode: ReplicationMode::SemiSync,
            shard_strategy: ShardStrategy::ConsistentHash,
            shard_count: 256,
            connection_pool_size: 32,
            heartbeat_interval_secs: 10,
            node_timeout_secs: 30,
            metrics_enabled: true,
            metrics_port: 9090,
            chunk_threshold: 64 * 1024 * 1024,
            chunk_size: 4 * 1024 * 1024,
            transfer_compression: true,
            read_load_balance: true,
            write_concern: 2,
        }
    }
}

/// Prometheus-style metric representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub help: String,
    pub metric_type: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn datacenter_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("datacenter")
}

fn nodes_dir(repo_root: &Path) -> std::path::PathBuf {
    datacenter_dir(repo_root).join("nodes")
}

fn load_cluster_config(repo_root: &Path) -> Result<ClusterConfig, LitError> {
    let path = datacenter_dir(repo_root).join("cluster.json");
    if path.exists() {
        let json = fs::read_to_string(&path).map_err(|e| LitError::io(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| LitError::general(format!("Parse cluster config: {}", e)))
    } else {
        Ok(ClusterConfig::default())
    }
}

fn save_cluster_config(repo_root: &Path, config: &ClusterConfig) -> Result<(), LitError> {
    let dir = datacenter_dir(repo_root);
    fs::create_dir_all(&dir).map_err(|e| LitError::io(e.to_string()))?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| LitError::general(format!("Serialize cluster config: {}", e)))?;
    fs::write(dir.join("cluster.json"), json).map_err(|e| LitError::io(e.to_string()))?;
    Ok(())
}

fn save_node(repo_root: &Path, node: &ClusterNode) -> Result<(), LitError> {
    let dir = nodes_dir(repo_root);
    fs::create_dir_all(&dir).map_err(|e| LitError::io(e.to_string()))?;
    let json = serde_json::to_string_pretty(node)
        .map_err(|e| LitError::general(format!("Serialize node: {}", e)))?;
    fs::write(dir.join(format!("{}.json", node.node_id)), json)
        .map_err(|e| LitError::io(e.to_string()))?;
    Ok(())
}

fn load_all_nodes(repo_root: &Path) -> Result<Vec<ClusterNode>, LitError> {
    let dir = nodes_dir(repo_root);
    let mut nodes = Vec::new();
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
                if let Ok(node) = serde_json::from_str::<ClusterNode>(&json) {
                    nodes.push(node);
                }
            }
        }
    }
    Ok(nodes)
}

/// Assign shard ranges to a node based on current cluster state
fn compute_shard_ranges(node_id: &str, all_nodes: &[ClusterNode], shard_count: u32) -> Vec<String> {
    let active_nodes: Vec<&ClusterNode> = all_nodes
        .iter()
        .filter(|n| n.health != HealthStatus::Unreachable && n.health != HealthStatus::Draining)
        .collect();

    if active_nodes.is_empty() {
        return (0..shard_count).map(|i| format!("{:02x}", i)).collect();
    }

    let pos = active_nodes
        .iter()
        .position(|n| n.node_id == node_id)
        .unwrap_or(active_nodes.len());

    let total = active_nodes.len() as u32;
    let shards_per_node = shard_count / total.max(1);
    let start = pos as u32 * shards_per_node;
    let end = if pos as u32 == total - 1 {
        shard_count
    } else {
        start + shards_per_node
    };

    (start..end).map(|i| format!("{:02x}", i % 256)).collect()
}

/// Collect Prometheus-style metrics for the local node
fn collect_metrics(repo_root: &Path) -> Vec<Metric> {
    let objects_dir = repo_root.join(".lit").join("objects");
    let mut object_count: u64 = 0;
    let mut total_size: u64 = 0;

    if let Ok(entries) = fs::read_dir(&objects_dir) {
        for shard in entries.flatten() {
            if shard.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(files) = fs::read_dir(shard.path()) {
                    for file in files.flatten() {
                        object_count += 1;
                        total_size += file.metadata().map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
    }

    let refs_count = repo_root
        .join(".lit")
        .join("refs")
        .read_dir()
        .map(|e| e.count() as u64)
        .unwrap_or(0);

    vec![
        Metric {
            name: "lit_objects_total".into(),
            help: "Total number of objects in the local store".into(),
            metric_type: "gauge".into(),
            value: object_count as f64,
            labels: HashMap::new(),
        },
        Metric {
            name: "lit_objects_size_bytes".into(),
            help: "Total size of all objects in bytes".into(),
            metric_type: "gauge".into(),
            value: total_size as f64,
            labels: HashMap::new(),
        },
        Metric {
            name: "lit_refs_total".into(),
            help: "Total number of refs".into(),
            metric_type: "gauge".into(),
            value: refs_count as f64,
            labels: HashMap::new(),
        },
    ]
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Show cluster status — nodes, shard distribution, config
pub fn execute_status() -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let config = load_cluster_config(&repo_root)?;
    let nodes = load_all_nodes(&repo_root)?;

    let healthy = nodes
        .iter()
        .filter(|n| n.health == HealthStatus::Healthy)
        .count();
    let total = nodes.len();

    Ok(DatacenterResponse {
        action: "status".into(),
        message: format!(
            "Cluster: {} node(s) ({} healthy), replication_factor={}, shards={}, strategy={}",
            total,
            healthy,
            config.replication_factor,
            config.shard_count,
            match config.shard_strategy {
                ShardStrategy::ConsistentHash => "consistent-hash",
                ShardStrategy::RangePrefix => "range-prefix",
                ShardStrategy::RoundRobin => "round-robin",
                ShardStrategy::DomainAffinity => "domain-affinity",
            }
        ),
        details: Some(serde_json::json!({
            "config": config,
            "nodes": nodes,
            "summary": {
                "total_nodes": total,
                "healthy_nodes": healthy,
                "total_storage": nodes.iter().map(|n| n.capacity.storage_total).sum::<u64>(),
                "used_storage": nodes.iter().map(|n| n.capacity.storage_used).sum::<u64>(),
                "total_objects": nodes.iter().map(|n| n.capacity.object_count).sum::<u64>(),
            }
        })),
    })
}

/// Register a new cluster node
pub fn execute_register_node(
    node_id: String,
    name: String,
    endpoint: String,
    region: String,
    role: Option<String>,
) -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let config = load_cluster_config(&repo_root)?;
    let existing = load_all_nodes(&repo_root)?;

    let role_enum = match role.as_deref() {
        Some("primary") => NodeRole::Primary,
        Some("replica") => NodeRole::Replica,
        Some("relay") => NodeRole::Relay,
        Some("observer") => NodeRole::Observer,
        _ => NodeRole::Replica,
    };

    let node = ClusterNode {
        node_id: node_id.clone(),
        name: name.clone(),
        endpoint,
        region: region.clone(),
        role: role_enum,
        health: HealthStatus::Bootstrapping,
        shard_ranges: compute_shard_ranges(&node_id, &existing, config.shard_count),
        last_heartbeat: Utc::now().to_rfc3339(),
        capacity: NodeCapacity {
            storage_total: 0,
            storage_used: 0,
            object_count: 0,
            max_connections: config.connection_pool_size,
            active_connections: 0,
            cpu_utilization: 0.0,
            memory_utilization: 0.0,
        },
        registered_at: Utc::now().to_rfc3339(),
    };

    save_node(&repo_root, &node)?;

    Ok(DatacenterResponse {
        action: "register-node".into(),
        message: format!(
            "Node '{}' ({}) registered in region '{}' with {} shard(s)",
            name,
            node_id,
            region,
            node.shard_ranges.len()
        ),
        details: Some(serde_json::to_value(&node).unwrap_or_default()),
    })
}

/// Configure cluster-level settings
pub fn execute_configure(
    replication_factor: Option<u32>,
    shard_count: Option<u32>,
    shard_strategy: Option<String>,
    replication_mode: Option<String>,
    connection_pool_size: Option<u32>,
    metrics_enabled: Option<bool>,
    metrics_port: Option<u16>,
    write_concern: Option<u32>,
) -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut config = load_cluster_config(&repo_root)?;

    if let Some(rf) = replication_factor {
        config.replication_factor = rf;
    }
    if let Some(sc) = shard_count {
        config.shard_count = sc;
    }
    if let Some(ss) = shard_strategy {
        config.shard_strategy = match ss.as_str() {
            "consistent-hash" => ShardStrategy::ConsistentHash,
            "range-prefix" => ShardStrategy::RangePrefix,
            "round-robin" => ShardStrategy::RoundRobin,
            "domain-affinity" => ShardStrategy::DomainAffinity,
            _ => ShardStrategy::ConsistentHash,
        };
    }
    if let Some(rm) = replication_mode {
        config.replication_mode = match rm.as_str() {
            "sync" | "synchronous" => ReplicationMode::Synchronous,
            "async" | "asynchronous" => ReplicationMode::Asynchronous,
            _ => ReplicationMode::SemiSync,
        };
    }
    if let Some(cps) = connection_pool_size {
        config.connection_pool_size = cps;
    }
    if let Some(me) = metrics_enabled {
        config.metrics_enabled = me;
    }
    if let Some(mp) = metrics_port {
        config.metrics_port = mp;
    }
    if let Some(wc) = write_concern {
        config.write_concern = wc;
    }

    save_cluster_config(&repo_root, &config)?;

    Ok(DatacenterResponse {
        action: "configure".into(),
        message: "Cluster configuration updated".into(),
        details: Some(serde_json::to_value(&config).unwrap_or_default()),
    })
}

/// Run health checks on all registered nodes
pub fn execute_health() -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let config = load_cluster_config(&repo_root)?;
    let nodes = load_all_nodes(&repo_root)?;

    let mut health_report: Vec<serde_json::Value> = Vec::new();
    let timeout_cutoff = Utc::now() - chrono::Duration::seconds(config.node_timeout_secs as i64);

    for node in &nodes {
        let last_hb = chrono::DateTime::parse_from_rfc3339(&node.last_heartbeat)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let effective_health = if last_hb < timeout_cutoff && node.health == HealthStatus::Healthy {
            HealthStatus::Unreachable
        } else {
            node.health.clone()
        };

        let storage_pct = if node.capacity.storage_total > 0 {
            (node.capacity.storage_used as f64 / node.capacity.storage_total as f64) * 100.0
        } else {
            0.0
        };

        health_report.push(serde_json::json!({
            "node_id": node.node_id,
            "name": node.name,
            "role": node.role.to_string(),
            "health": effective_health.to_string(),
            "region": node.region,
            "storage_pct": format!("{:.1}%", storage_pct),
            "cpu": format!("{:.1}%", node.capacity.cpu_utilization * 100.0),
            "memory": format!("{:.1}%", node.capacity.memory_utilization * 100.0),
            "connections": format!("{}/{}", node.capacity.active_connections, node.capacity.max_connections),
            "last_heartbeat": node.last_heartbeat,
        }));
    }

    Ok(DatacenterResponse {
        action: "health".into(),
        message: format!("Health check for {} node(s)", nodes.len()),
        details: Some(serde_json::to_value(&health_report).unwrap_or_default()),
    })
}

/// Collect and return Prometheus-style metrics
pub fn execute_metrics() -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let metrics = collect_metrics(&repo_root);

    // Also include per-node metrics if cluster is configured
    let nodes = load_all_nodes(&repo_root)?;
    let mut all_metrics = metrics;

    all_metrics.push(Metric {
        name: "lit_cluster_nodes_total".into(),
        help: "Total nodes in cluster".into(),
        metric_type: "gauge".into(),
        value: nodes.len() as f64,
        labels: HashMap::new(),
    });

    let healthy = nodes
        .iter()
        .filter(|n| n.health == HealthStatus::Healthy)
        .count();
    all_metrics.push(Metric {
        name: "lit_cluster_nodes_healthy".into(),
        help: "Healthy nodes in cluster".into(),
        metric_type: "gauge".into(),
        value: healthy as f64,
        labels: HashMap::new(),
    });

    // Render as Prometheus exposition format
    let exposition: String = all_metrics
        .iter()
        .map(|m| {
            let labels_str = if m.labels.is_empty() {
                String::new()
            } else {
                let pairs: Vec<String> = m
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            };
            format!(
                "# HELP {} {}\n# TYPE {} {}\n{}{} {}",
                m.name, m.help, m.name, m.metric_type, m.name, labels_str, m.value
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(DatacenterResponse {
        action: "metrics".into(),
        message: format!("{} metric(s) collected", all_metrics.len()),
        details: Some(serde_json::json!({
            "metrics": all_metrics,
            "exposition": exposition,
        })),
    })
}

/// Remove a node from the cluster (drain first recommended)
pub fn execute_remove_node(node_id: String) -> Result<DatacenterResponse, LitError> {
    let repo_root = find_repo_root()?;
    let dir = nodes_dir(&repo_root);
    let path = dir.join(format!("{}.json", node_id));

    if !path.exists() {
        return Err(LitError::general(format!("Node not found: {}", node_id)));
    }

    fs::remove_file(&path).map_err(|e| LitError::io(e.to_string()))?;

    Ok(DatacenterResponse {
        action: "remove-node".into(),
        message: format!("Node '{}' removed from cluster", node_id),
        details: None,
    })
}
