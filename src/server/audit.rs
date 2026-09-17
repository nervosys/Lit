//! Server audit events.
//!
//! `lit serve` previously wrote nothing to the audit log: the HMAC-chained log
//! in `network::audit` existed, but only the transport layer fed it. This
//! module is the server's side of that wiring.
//!
//! NIST SP 800-171r3 requirements implemented here:
//!
//! - `03.03.01` event logging — the event types below are the logged set
//! - `03.03.02` audit record content — every record carries what happened,
//!   when, from where, by whom, and the outcome
//! - `03.03.04` response to logging failures — a failed write is surfaced to
//!   the operator on stderr rather than dropped
//! - `03.03.08` protection of audit information — records are HMAC-signed and
//!   the log is owner-only, both inherited from `network::audit::AuditLog`
//!
//! An audit record is only as good as its subject field. Because the server now
//! authenticates named accounts, `subject` is a username rather than "a client
//! that presented the token".

use crate::network::audit::AuditLog;
use serde::Serialize;
use std::net::IpAddr;

/// The set of server events that are logged.
///
/// This enum *is* the organization-defined event list for `03.03.01`; adding a
/// variant is the way to extend it, and the `as_str` names are what appear in
/// the log and in queries against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerEvent {
    /// The server began listening.
    ServerStart,
    /// The server stopped listening.
    ServerStop,
    /// Credentials verified and a session issued.
    AuthSuccess,
    /// Credentials rejected.
    AuthFailure,
    /// An account crossed the lockout threshold.
    AuthLockout,
    /// A request presented a session that had expired or was unknown.
    SessionInvalid,
    /// A session was explicitly terminated (logout, or administrative action).
    SessionEnd,
    /// An authenticated caller was refused for lack of privilege.
    AccessDenied,
    /// An authorized request was served.
    ApiRequest,
    /// An account was created, modified, disabled, or removed.
    AccountChange,
    /// A request was refused by the rate limiter.
    RateLimited,
    /// A request was refused before authentication (malformed, oversized).
    RequestRejected,
}

impl ServerEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerEvent::ServerStart => "SERVER_START",
            ServerEvent::ServerStop => "SERVER_STOP",
            ServerEvent::AuthSuccess => "AUTH_SUCCESS",
            ServerEvent::AuthFailure => "AUTH_FAILURE",
            ServerEvent::AuthLockout => "AUTH_LOCKOUT",
            ServerEvent::SessionInvalid => "SESSION_INVALID",
            ServerEvent::SessionEnd => "SESSION_END",
            ServerEvent::AccessDenied => "ACCESS_DENIED",
            ServerEvent::ApiRequest => "API_REQUEST",
            ServerEvent::AccountChange => "ACCOUNT_CHANGE",
            ServerEvent::RateLimited => "RATE_LIMITED",
            ServerEvent::RequestRejected => "REQUEST_REJECTED",
        }
    }
}

/// The content of one audit record (`03.03.02`).
#[derive(Debug, Clone, Serialize, Default)]
pub struct AuditRecord {
    /// The account responsible, or `None` before authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Where the request came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// What was acted on — a route, an account name, a session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// `success` or `failure`, plus a reason where one applies.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditRecord {
    /// A record for an action that succeeded.
    pub fn success() -> Self {
        AuditRecord {
            outcome: "success",
            ..Default::default()
        }
    }

    /// A record for an action that was refused or failed.
    pub fn failure(reason: impl Into<String>) -> Self {
        AuditRecord {
            outcome: "failure",
            reason: Some(reason.into()),
            ..Default::default()
        }
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn maybe_subject(mut self, subject: Option<String>) -> Self {
        self.subject = subject;
        self
    }

    pub fn source(mut self, source: Option<IpAddr>) -> Self {
        self.source = source.map(|ip| ip.to_string());
        self
    }

    pub fn object(mut self, object: impl Into<String>) -> Self {
        self.object = Some(object.into());
        self
    }
}

/// The server's audit sink.
///
/// Holds an `AuditLog` when auditing is enabled. When it is disabled the
/// recorder is inert, so that call sites need no conditional of their own.
pub struct AuditRecorder {
    log: Option<AuditLog>,
}

impl AuditRecorder {
    /// Open the audit log. `path` of `None` uses the default location.
    pub fn enabled(path: Option<&str>) -> Result<Self, String> {
        Ok(AuditRecorder {
            log: Some(AuditLog::new(path)?),
        })
    }

    /// A recorder that discards events, for when auditing is switched off.
    pub fn disabled() -> Self {
        AuditRecorder { log: None }
    }

    /// Whether events are actually being written.
    pub fn is_enabled(&self) -> bool {
        self.log.is_some()
    }

    /// Write one record.
    ///
    /// A failure to write is reported on stderr and otherwise swallowed
    /// (`03.03.04`). Refusing to serve because the audit log is unwritable is
    /// the stricter response and some deployments require it; that choice
    /// belongs to the operator, and is documented rather than imposed here.
    pub fn record(&self, event: ServerEvent, record: AuditRecord) {
        let Some(log) = &self.log else {
            return;
        };
        let message = serde_json::to_string(&record)
            .unwrap_or_else(|_| r#"{"outcome":"failure","reason":"unserializable record"}"#.to_string());
        if let Err(e) = log.log(event.as_str(), &message) {
            eprintln!(
                "AUDIT FAILURE: could not record {}: {} — the event above is not in the log",
                event.as_str(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disabled_recorder_accepts_events_without_writing() {
        let recorder = AuditRecorder::disabled();
        assert!(!recorder.is_enabled());
        recorder.record(ServerEvent::AuthSuccess, AuditRecord::success().subject("alice"));
    }

    #[test]
    fn records_carry_the_fields_the_control_requires() {
        let record = AuditRecord::failure("invalid_credentials")
            .subject("alice")
            .source(Some("192.0.2.10".parse().unwrap()))
            .object("/api/v1/auth/login");
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"subject\":\"alice\""));
        assert!(json.contains("\"source\":\"192.0.2.10\""));
        assert!(json.contains("\"object\":\"/api/v1/auth/login\""));
        assert!(json.contains("\"outcome\":\"failure\""));
        assert!(json.contains("\"reason\":\"invalid_credentials\""));
    }

    #[test]
    fn a_pre_authentication_record_omits_the_subject_rather_than_inventing_one() {
        let record = AuditRecord::failure("rate_limited").source(Some("192.0.2.10".parse().unwrap()));
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("subject"));
    }

    #[test]
    fn every_event_has_a_distinct_name() {
        let events = [
            ServerEvent::ServerStart,
            ServerEvent::ServerStop,
            ServerEvent::AuthSuccess,
            ServerEvent::AuthFailure,
            ServerEvent::AuthLockout,
            ServerEvent::SessionInvalid,
            ServerEvent::SessionEnd,
            ServerEvent::AccessDenied,
            ServerEvent::ApiRequest,
            ServerEvent::AccountChange,
            ServerEvent::RateLimited,
            ServerEvent::RequestRejected,
        ];
        let names: std::collections::HashSet<&str> = events.iter().map(|e| e.as_str()).collect();
        assert_eq!(names.len(), events.len());
    }
}
