//! Agent Trust Scoring Engine
//!
//! Tracks reputation, reliability, and trust of agents based on observable
//! behavior: commits, reviews, merges, violations, and delegation outcomes.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Events that affect an agent's trust score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustEvent {
    /// Agent committed code
    Commit { hash: String },
    /// Agent performed a code review
    Review { target_hash: String },
    /// Agent merged a branch
    Merge { branch: String },
    /// Agent's delegated task was completed successfully
    DelegationCompleted { task_id: String },
    /// Agent's delegated task failed or was abandoned
    DelegationFailed { task_id: String },
    /// Agent violated a policy (e.g., force-push to protected branch)
    Violation { description: String },
    /// Agent's token was revoked for cause
    TokenRevoked { reason: String },
    /// Peer vouched for this agent
    PeerVouch { voucher_did: String },
}

impl TrustEvent {
    /// Score impact of this event
    fn score_delta(&self) -> f64 {
        match self {
            TrustEvent::Commit { .. } => 1.0,
            TrustEvent::Review { .. } => 2.0,
            TrustEvent::Merge { .. } => 1.5,
            TrustEvent::DelegationCompleted { .. } => 3.0,
            TrustEvent::DelegationFailed { .. } => -2.0,
            TrustEvent::Violation { .. } => -10.0,
            TrustEvent::TokenRevoked { .. } => -5.0,
            TrustEvent::PeerVouch { .. } => 2.5,
        }
    }
}

/// Trust score record for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    /// Agent's DID
    pub did: String,
    /// Current trust score (0.0 to 100.0, clamped)
    pub score: f64,
    /// Total events recorded
    pub total_events: u64,
    /// Trust level derived from score
    pub level: TrustLevel,
    /// Event history
    pub events: Vec<TrustEventRecord>,
    /// Last updated timestamp
    pub updated: String,
}

/// Timestamped trust event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEventRecord {
    pub event: TrustEvent,
    pub timestamp: String,
    pub score_delta: f64,
}

/// Trust level labels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrustLevel {
    Untrusted,
    Newcomer,
    Contributor,
    Trusted,
    Maintainer,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Untrusted => write!(f, "Untrusted"),
            TrustLevel::Newcomer => write!(f, "Newcomer"),
            TrustLevel::Contributor => write!(f, "Contributor"),
            TrustLevel::Trusted => write!(f, "Trusted"),
            TrustLevel::Maintainer => write!(f, "Maintainer"),
        }
    }
}

fn level_for_score(score: f64) -> TrustLevel {
    if score < 10.0 {
        TrustLevel::Untrusted
    } else if score < 30.0 {
        TrustLevel::Newcomer
    } else if score < 60.0 {
        TrustLevel::Contributor
    } else if score < 85.0 {
        TrustLevel::Trusted
    } else {
        TrustLevel::Maintainer
    }
}

/// Trust scoring engine
pub struct TrustEngine {
    repo_root: std::path::PathBuf,
}

impl TrustEngine {
    pub fn new(repo_root: &Path) -> Self {
        TrustEngine {
            repo_root: repo_root.to_path_buf(),
        }
    }

    fn trust_dir(&self) -> std::path::PathBuf {
        self.repo_root.join(".lit").join("trust")
    }

    fn score_path(&self, did: &str) -> std::path::PathBuf {
        // Hash DID for safe filename
        let safe_name: String = did
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        self.trust_dir().join(format!("{}.json", safe_name))
    }

    /// Get or initialize a trust score for an agent
    pub fn get_score(&self, did: &str) -> Result<TrustScore, LitError> {
        let path = self.score_path(did);
        if path.exists() {
            let json = fs::read_to_string(&path)
                .map_err(|e| LitError::io(format!("Failed to read trust score: {}", e)))?;
            serde_json::from_str(&json)
                .map_err(|e| LitError::general(format!("Failed to parse trust score: {}", e)))
        } else {
            Ok(TrustScore {
                did: did.to_string(),
                score: 25.0, // Start as newcomer
                total_events: 0,
                level: TrustLevel::Newcomer,
                events: Vec::new(),
                updated: chrono::Utc::now().to_rfc3339(),
            })
        }
    }

    /// Record a trust event for an agent
    pub fn record_event(&self, did: &str, event: TrustEvent) -> Result<TrustScore, LitError> {
        let mut score = self.get_score(did)?;
        let delta = event.score_delta();

        score.events.push(TrustEventRecord {
            event,
            timestamp: chrono::Utc::now().to_rfc3339(),
            score_delta: delta,
        });

        score.score = (score.score + delta).clamp(0.0, 100.0);
        score.total_events += 1;
        score.level = level_for_score(score.score);
        score.updated = chrono::Utc::now().to_rfc3339();

        self.save_score(&score)?;
        Ok(score)
    }

    /// List all known agents with trust scores
    pub fn list_agents(&self) -> Result<Vec<TrustScore>, LitError> {
        let dir = self.trust_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut scores = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO error: {}", e)))? {
            let entry = entry.map_err(|e| LitError::io(format!("IO error: {}", e)))?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                if let Ok(json) = fs::read_to_string(entry.path()) {
                    if let Ok(score) = serde_json::from_str::<TrustScore>(&json) {
                        scores.push(score);
                    }
                }
            }
        }

        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(scores)
    }

    fn save_score(&self, score: &TrustScore) -> Result<(), LitError> {
        let dir = self.trust_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| LitError::io(format!("Failed to create trust dir: {}", e)))?;

        let path = self.score_path(&score.did);
        let json = serde_json::to_string_pretty(score)
            .map_err(|e| LitError::general(format!("Failed to serialize trust score: {}", e)))?;
        fs::write(&path, json)
            .map_err(|e| LitError::io(format!("Failed to write trust score: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lit_trust_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_trust_score_default() {
        let dir = tmp_dir();
        let engine = TrustEngine::new(&dir);
        let score = engine.get_score("did:lit:agent1").unwrap();
        assert_eq!(score.score, 25.0);
        assert_eq!(score.level, TrustLevel::Newcomer);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_trust_score_events() {
        let dir = tmp_dir();
        let engine = TrustEngine::new(&dir);

        let score = engine
            .record_event("did:lit:x", TrustEvent::Commit { hash: "abc".into() })
            .unwrap();
        assert_eq!(score.score, 26.0);

        let score = engine
            .record_event(
                "did:lit:x",
                TrustEvent::Violation {
                    description: "force-push".into(),
                },
            )
            .unwrap();
        assert_eq!(score.score, 16.0);
        assert_eq!(score.level, TrustLevel::Newcomer);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_trust_level_boundaries() {
        assert_eq!(level_for_score(0.0), TrustLevel::Untrusted);
        assert_eq!(level_for_score(25.0), TrustLevel::Newcomer);
        assert_eq!(level_for_score(50.0), TrustLevel::Contributor);
        assert_eq!(level_for_score(75.0), TrustLevel::Trusted);
        assert_eq!(level_for_score(95.0), TrustLevel::Maintainer);
    }
}
