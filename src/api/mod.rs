//! Self-Describing API (JSON-LD / Hydra)
//!
//! Adds hypermedia links and JSON-LD context to lit's API responses,
//! making the API discoverable and self-documenting for agents.

use serde::{Deserialize, Serialize};

/// JSON-LD context for lit API responses
pub const LIT_CONTEXT: &str = "https://lit.nervosys.com/api/v1/context.jsonld";

/// A hypermedia link (Hydra-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLink {
    /// Relation type (e.g., "self", "next", "parent", "children")
    pub rel: String,
    /// Target URI or path
    pub href: String,
    /// HTTP method (GET, POST, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Human-readable title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Self-describing API envelope that wraps any response with JSON-LD context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvelope<T: Serialize> {
    /// JSON-LD context
    #[serde(rename = "@context")]
    pub context: String,
    /// Resource type
    #[serde(rename = "@type")]
    pub resource_type: String,
    /// Hypermedia links for discoverability
    #[serde(rename = "_links")]
    pub links: Vec<ApiLink>,
    /// The actual response data
    pub data: T,
}

impl<T: Serialize> ApiEnvelope<T> {
    /// Wrap a response with JSON-LD context and links
    pub fn wrap(resource_type: &str, data: T, links: Vec<ApiLink>) -> Self {
        ApiEnvelope {
            context: LIT_CONTEXT.to_string(),
            resource_type: resource_type.to_string(),
            links,
            data,
        }
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Generate standard links for a repository resource
pub fn repo_links(repo_name: &str) -> Vec<ApiLink> {
    vec![
        ApiLink {
            rel: "self".to_string(),
            href: format!("/repos/{}", repo_name),
            method: Some("GET".to_string()),
            title: Some("This repository".to_string()),
        },
        ApiLink {
            rel: "commits".to_string(),
            href: format!("/repos/{}/commits", repo_name),
            method: Some("GET".to_string()),
            title: Some("List commits".to_string()),
        },
        ApiLink {
            rel: "branches".to_string(),
            href: format!("/repos/{}/branches", repo_name),
            method: Some("GET".to_string()),
            title: Some("List branches".to_string()),
        },
        ApiLink {
            rel: "issues".to_string(),
            href: format!("/repos/{}/issues", repo_name),
            method: Some("GET".to_string()),
            title: Some("List issues".to_string()),
        },
        ApiLink {
            rel: "prs".to_string(),
            href: format!("/repos/{}/prs", repo_name),
            method: Some("GET".to_string()),
            title: Some("List pull requests".to_string()),
        },
        ApiLink {
            rel: "peers".to_string(),
            href: format!("/repos/{}/peers", repo_name),
            method: Some("GET".to_string()),
            title: Some("List federated peers".to_string()),
        },
        ApiLink {
            rel: "events".to_string(),
            href: format!("/repos/{}/events", repo_name),
            method: Some("GET".to_string()),
            title: Some("Event stream".to_string()),
        },
    ]
}

/// Generate links for a commit resource
pub fn commit_links(repo_name: &str, commit_hash: &str) -> Vec<ApiLink> {
    vec![
        ApiLink {
            rel: "self".to_string(),
            href: format!("/repos/{}/commits/{}", repo_name, commit_hash),
            method: Some("GET".to_string()),
            title: Some("This commit".to_string()),
        },
        ApiLink {
            rel: "tree".to_string(),
            href: format!("/repos/{}/tree/{}", repo_name, commit_hash),
            method: Some("GET".to_string()),
            title: Some("File tree at this commit".to_string()),
        },
        ApiLink {
            rel: "parent".to_string(),
            href: format!("/repos/{}/commits/{}~1", repo_name, commit_hash),
            method: Some("GET".to_string()),
            title: Some("Parent commit".to_string()),
        },
        ApiLink {
            rel: "diff".to_string(),
            href: format!("/repos/{}/diff/{}", repo_name, commit_hash),
            method: Some("GET".to_string()),
            title: Some("Diff for this commit".to_string()),
        },
    ]
}

/// Generate links for a DID identity resource
pub fn identity_links(did: &str) -> Vec<ApiLink> {
    let safe_did = did.replace(':', "_");
    vec![
        ApiLink {
            rel: "self".to_string(),
            href: format!("/identities/{}", safe_did),
            method: Some("GET".to_string()),
            title: Some("This identity".to_string()),
        },
        ApiLink {
            rel: "trust".to_string(),
            href: format!("/identities/{}/trust", safe_did),
            method: Some("GET".to_string()),
            title: Some("Trust score".to_string()),
        },
        ApiLink {
            rel: "tokens".to_string(),
            href: format!("/identities/{}/tokens", safe_did),
            method: Some("GET".to_string()),
            title: Some("UCAN tokens".to_string()),
        },
        ApiLink {
            rel: "delegations".to_string(),
            href: format!("/identities/{}/delegations", safe_did),
            method: Some("GET".to_string()),
            title: Some("Task delegations".to_string()),
        },
    ]
}

/// API root entry point with all discoverable endpoints
pub fn api_root_links() -> Vec<ApiLink> {
    vec![
        ApiLink {
            rel: "self".to_string(),
            href: "/".to_string(),
            method: Some("GET".to_string()),
            title: Some("API root".to_string()),
        },
        ApiLink {
            rel: "repos".to_string(),
            href: "/repos".to_string(),
            method: Some("GET".to_string()),
            title: Some("List repositories".to_string()),
        },
        ApiLink {
            rel: "identities".to_string(),
            href: "/identities".to_string(),
            method: Some("GET".to_string()),
            title: Some("List identities".to_string()),
        },
        ApiLink {
            rel: "events".to_string(),
            href: "/events".to_string(),
            method: Some("GET".to_string()),
            title: Some("Global event stream".to_string()),
        },
        ApiLink {
            rel: "federation".to_string(),
            href: "/federation".to_string(),
            method: Some("GET".to_string()),
            title: Some("Federation status".to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_envelope() {
        let data = serde_json::json!({"name": "test-repo"});
        let envelope = ApiEnvelope::wrap("Repository", data, repo_links("test-repo"));
        let json = envelope.to_json().unwrap();
        assert!(json.contains("@context"));
        assert!(json.contains("@type"));
        assert!(json.contains("_links"));
        assert!(json.contains("test-repo"));
    }

    #[test]
    fn test_repo_links() {
        let links = repo_links("myrepo");
        assert!(links.iter().any(|l| l.rel == "self"));
        assert!(links.iter().any(|l| l.rel == "commits"));
        assert!(links.iter().any(|l| l.rel == "issues"));
    }

    #[test]
    fn test_api_root() {
        let links = api_root_links();
        assert!(links.iter().any(|l| l.rel == "repos"));
        assert!(links.iter().any(|l| l.rel == "identities"));
        assert!(links.iter().any(|l| l.rel == "federation"));
    }
}
