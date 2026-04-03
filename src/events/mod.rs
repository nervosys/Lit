//! Event Subscription System
//!
//! Allows agents and users to subscribe to repo-level events:
//! commits, branch updates, merges, issues, PRs, agent joins, task delegations.
//! Upgrades `lit watch` with structured, filterable event streams.

pub mod subscription;
