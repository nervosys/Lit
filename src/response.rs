use serde::{Deserialize, Serialize};

/// Output format for command responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Human,
}

impl OutputFormat {
    /// Determine output format from CLI flags and environment
    pub fn resolve(json: bool, human: bool) -> Self {
        if human {
            return OutputFormat::Human;
        }
        if json {
            return OutputFormat::Json;
        }
        // Check environment variable
        match std::env::var("LIT_OUTPUT").as_deref() {
            Ok("human") => OutputFormat::Human,
            _ => OutputFormat::Json, // Default: JSON (agent-first)
        }
    }
}

/// Unified response wrapper for all command output
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandOutput {
    pub status: &'static str,
    pub command: &'static str,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Trait for command responses that can be rendered in multiple formats
pub trait CommandResponse: Serialize {
    /// The command name for the response envelope
    fn command_name(&self) -> &'static str;

    /// Render as human-readable text
    fn human_readable(&self) -> String;

    /// Render as JSON (default implementation via serde)
    fn to_json_output(&self) -> String {
        let data = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let output = CommandOutput {
            status: "ok",
            command: self.command_name(),
            data,
        };
        serde_json::to_string_pretty(&output).unwrap_or_default()
    }
}

/// Render a response in the specified format
pub fn render<R: CommandResponse>(response: &R, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => response.to_json_output(),
        OutputFormat::Human => response.human_readable(),
    }
}

/// Render an error in the specified format
pub fn render_error(
    error: &crate::errors::LitError,
    command: &str,
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Json => {
            let err_obj = serde_json::json!({
                "status": "error",
                "command": command,
                "error": {
                    "code": error.error_code(),
                    "message": error.user_message(),
                    "suggestions": error.suggestions(),
                }
            });
            serde_json::to_string_pretty(&err_obj).unwrap_or_default()
        }
        OutputFormat::Human => {
            let mut out = format!("error: {}", error.user_message());
            let suggestions = error.suggestions();
            if !suggestions.is_empty() {
                out.push_str("\n\nhint:");
                for s in suggestions {
                    out.push_str(&format!("\n  {}", s));
                }
            }
            out
        }
    }
}

// â”€â”€â”€ Response types â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Serialize, Deserialize)]
pub struct InitResponse {
    pub path: String,
    pub bare: bool,
}

impl CommandResponse for InitResponse {
    fn command_name(&self) -> &'static str {
        "init"
    }
    fn human_readable(&self) -> String {
        if self.bare {
            format!("Initialized empty bare Lit repository in {}", self.path)
        } else {
            format!("Initialized empty Lit repository in {}", self.path)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddResponse {
    pub files_added: usize,
}

impl CommandResponse for AddResponse {
    fn command_name(&self) -> &'static str {
        "add"
    }
    fn human_readable(&self) -> String {
        format!("Added {} file(s) to staging area", self.files_added)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitResponse {
    pub hash: String,
    pub short_hash: String,
    pub tree: String,
    pub parent: Option<String>,
    pub author: String,
    pub message: String,
    pub timestamp: i64,
}

impl CommandResponse for CommitResponse {
    fn command_name(&self) -> &'static str {
        "commit"
    }
    fn human_readable(&self) -> String {
        format!("[{}] {}", self.short_hash, self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
    pub clean: bool,
}

impl CommandResponse for StatusResponse {
    fn command_name(&self) -> &'static str {
        "status"
    }
    fn human_readable(&self) -> String {
        // ANSI color codes (matches git's palette)
        const GREEN: &str = "\x1b[32m";
        const ORANGE: &str = "\x1b[33m";
        const RED: &str = "\x1b[31m";
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";

        let mut out = String::new();
        if let Some(branch) = &self.branch {
            out.push_str(&format!("On branch {BOLD}{branch}{RESET}\n"));
        } else {
            out.push_str(&format!("{BOLD}HEAD detached{RESET}\n"));
        }

        if self.clean {
            out.push_str("nothing to commit, working tree clean\n");
            return out;
        }

        if !self.staged.is_empty() {
            out.push_str(&format!("\n{BOLD}Changes to be committed:{RESET}\n"));
            for f in &self.staged {
                out.push_str(&format!("{GREEN}  new file:   {f}{RESET}\n"));
            }
        }
        if !self.modified.is_empty() {
            out.push_str(&format!("\n{BOLD}Changes not staged for commit:{RESET}\n"));
            for f in &self.modified {
                out.push_str(&format!("{ORANGE}  modified:   {f}{RESET}\n"));
            }
        }
        if !self.untracked.is_empty() {
            out.push_str(&format!("\n{BOLD}Untracked files:{RESET}\n"));
            for f in &self.untracked {
                out.push_str(&format!("{RED}  {f}{RESET}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub timestamp: i64,
    pub message: String,
    pub is_head: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogResponse {
    pub branch: Option<String>,
    pub commits: Vec<CommitEntry>,
}

impl CommandResponse for LogResponse {
    fn command_name(&self) -> &'static str {
        "log"
    }
    fn human_readable(&self) -> String {
        if self.commits.is_empty() {
            return "No commits yet\n".to_string();
        }
        let mut out = String::new();
        for entry in &self.commits {
            out.push_str(&format!("commit {}\n", entry.hash));
            if entry.is_head {
                if let Some(branch) = &self.branch {
                    out.push_str(&format!("  (HEAD -> {})\n", branch));
                }
            }
            out.push_str(&format!("Author: {}\n", entry.author));
            if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(entry.timestamp, 0) {
                out.push_str(&format!(
                    "Date:   {}\n",
                    dt.format("%a %b %d %H:%M:%S %Y %z")
                ));
            }
            out.push('\n');
            for line in entry.message.lines() {
                out.push_str(&format!("    {}\n", line));
            }
            out.push('\n');
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BranchEntry {
    pub name: String,
    pub is_current: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum BranchResponse {
    #[serde(rename = "list")]
    List { branches: Vec<BranchEntry> },
    #[serde(rename = "create")]
    Create { name: String },
    #[serde(rename = "delete")]
    Delete { name: String },
}

impl CommandResponse for BranchResponse {
    fn command_name(&self) -> &'static str {
        "branch"
    }
    fn human_readable(&self) -> String {
        match self {
            BranchResponse::List { branches } => {
                if branches.is_empty() {
                    return "No branches yet\n".to_string();
                }
                let mut out = String::new();
                for b in branches {
                    let marker = if b.is_current { "* " } else { "  " };
                    out.push_str(&format!("{}{}\n", marker, b.name));
                }
                out
            }
            BranchResponse::Create { name } => format!("Created branch '{}'\n", name),
            BranchResponse::Delete { name } => format!("Deleted branch '{}'\n", name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutResponse {
    pub target: String,
    pub is_new_branch: bool,
    pub is_detached: bool,
}

impl CommandResponse for CheckoutResponse {
    fn command_name(&self) -> &'static str {
        "checkout"
    }
    fn human_readable(&self) -> String {
        if self.is_new_branch {
            format!("Switched to a new branch '{}'\n", self.target)
        } else if self.is_detached {
            format!(
                "HEAD is now at {} (detached)\n",
                &self.target[..16.min(self.target.len())]
            )
        } else {
            format!("Switched to branch '{}'\n", self.target)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "object_type")]
pub enum ShowResponse {
    #[serde(rename = "commit")]
    Commit {
        hash: String,
        author: String,
        timestamp: i64,
        message: String,
    },
    #[serde(rename = "tree")]
    Tree {
        hash: String,
        entries: Vec<TreeEntryInfo>,
    },
    #[serde(rename = "blob")]
    Blob {
        hash: String,
        size: usize,
        content: Option<String>,
        is_binary: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeEntryInfo {
    pub mode: String,
    pub object_type: String,
    pub hash: String,
    pub name: String,
}

impl CommandResponse for ShowResponse {
    fn command_name(&self) -> &'static str {
        "show"
    }
    fn human_readable(&self) -> String {
        match self {
            ShowResponse::Commit {
                hash,
                author,
                timestamp,
                message,
            } => {
                let mut out = format!("commit {}\nAuthor: {}\n", hash, author);
                if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(*timestamp, 0) {
                    out.push_str(&format!(
                        "Date:   {}\n",
                        dt.format("%a %b %d %H:%M:%S %Y %z")
                    ));
                }
                out.push_str(&format!("\n{}\n", message));
                out
            }
            ShowResponse::Tree { hash, entries } => {
                let mut out = format!("tree {}\n\n", hash);
                for e in entries {
                    out.push_str(&format!(
                        "{} {} {}\t{}\n",
                        e.mode,
                        e.object_type,
                        &e.hash[..16.min(e.hash.len())],
                        e.name
                    ));
                }
                out
            }
            ShowResponse::Blob {
                hash,
                size,
                content,
                is_binary,
            } => {
                let mut out = format!("blob {}\n\n", hash);
                if *is_binary {
                    out.push_str(&format!("(binary content, {} bytes)\n", size));
                } else if let Some(text) = content {
                    out.push_str(text);
                    out.push('\n');
                }
                out
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum RemoteResponse {
    #[serde(rename = "list")]
    List { remotes: Vec<RemoteEntry> },
    #[serde(rename = "add")]
    Add { name: String, url: String },
    #[serde(rename = "remove")]
    Remove { name: String },
}

impl CommandResponse for RemoteResponse {
    fn command_name(&self) -> &'static str {
        "remote"
    }
    fn human_readable(&self) -> String {
        match self {
            RemoteResponse::List { remotes } => {
                if remotes.is_empty() {
                    return "No remotes configured\n".to_string();
                }
                let mut out = String::new();
                for r in remotes {
                    out.push_str(&format!("{}\t{}\n", r.name, r.url));
                }
                out
            }
            RemoteResponse::Add { name, .. } => format!("Added remote '{}'\n", name),
            RemoteResponse::Remove { name } => format!("Removed remote '{}'\n", name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum ConfigResponse {
    #[serde(rename = "show")]
    Show { entries: Vec<ConfigEntry> },
    #[serde(rename = "get")]
    Get { key: String, value: String },
    #[serde(rename = "set")]
    Set { key: String, value: String },
}

impl CommandResponse for ConfigResponse {
    fn command_name(&self) -> &'static str {
        "config"
    }
    fn human_readable(&self) -> String {
        match self {
            ConfigResponse::Show { entries } => {
                let mut out = String::from("Lit Configuration\n==================\n\n");
                for e in entries {
                    out.push_str(&format!("{} = {}\n", e.key, e.value));
                }
                out
            }
            ConfigResponse::Get { key, value } => format!("{} = {}\n", key, value),
            ConfigResponse::Set { key, value } => format!("Set {} = {}\n", key, value),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeResponse {
    pub merged: bool,
    pub fast_forward: bool,
    pub commit_hash: Option<String>,
    pub message: String,
    pub has_conflicts: bool,
    pub file_results: Vec<FileMergeInfo>,
    pub strategy: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMergeInfo {
    pub path: String,
    pub status: String,
    pub conflict_count: usize,
}

impl CommandResponse for MergeResponse {
    fn command_name(&self) -> &'static str {
        "merge"
    }
    fn human_readable(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.message);
        out.push('\n');

        if !self.file_results.is_empty() {
            for f in &self.file_results {
                let icon = match f.status.as_str() {
                    "conflict" => "C",
                    "added" => "A",
                    "deleted" => "D",
                    "autoresolved" => "M",
                    _ => " ",
                };
                out.push_str(&format!("  {} {}\n", icon, f.path));
            }
        }

        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub resolved_files: Vec<String>,
    pub remaining_conflicts: usize,
    pub merge_complete: bool,
    pub message: String,
}

impl CommandResponse for ResolveResponse {
    fn command_name(&self) -> &'static str {
        "resolve"
    }
    fn human_readable(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.message);
        out.push('\n');

        for f in &self.resolved_files {
            out.push_str(&format!("  Resolved: {}\n", f));
        }

        if self.remaining_conflicts > 0 {
            out.push_str(&format!(
                "  {} conflict(s) remaining\n",
                self.remaining_conflicts
            ));
        }

        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub remote: String,
    pub branch: String,
    pub objects_transferred: usize,
    pub updated: bool,
    pub message: String,
}

impl CommandResponse for PushResponse {
    fn command_name(&self) -> &'static str {
        "push"
    }
    fn human_readable(&self) -> String {
        format!("{}\n", self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub remote: String,
    pub branch: String,
    pub objects_fetched: usize,
    pub fast_forward: bool,
    pub has_conflicts: bool,
    pub merge_message: String,
    pub message: String,
}

impl CommandResponse for PullResponse {
    fn command_name(&self) -> &'static str {
        "pull"
    }
    fn human_readable(&self) -> String {
        format!("{}\n", self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneResponse {
    pub url: String,
    pub directory: String,
    pub branches_cloned: Vec<String>,
    pub objects_transferred: usize,
    pub message: String,
}

impl CommandResponse for CloneResponse {
    fn command_name(&self) -> &'static str {
        "clone"
    }
    fn human_readable(&self) -> String {
        format!("{}\n", self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResponse {
    pub remote: String,
    pub branches_updated: Vec<String>,
    pub objects_transferred: usize,
    pub message: String,
}

impl CommandResponse for FetchResponse {
    fn command_name(&self) -> &'static str {
        "fetch"
    }
    fn human_readable(&self) -> String {
        format!("{}\n", self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResponse {
    pub files: Vec<crate::core::diff::FileDiff>,
    pub stats: Vec<crate::core::diff::DiffStat>,
    pub stat_only: bool,
    pub word_diff: bool,
    pub files_changed: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
}

impl CommandResponse for DiffResponse {
    fn command_name(&self) -> &'static str {
        "diff"
    }
    fn human_readable(&self) -> String {
        use crate::core::diff::{annotate_hunk_word_diff, DiffLineKind, FileStatus};

        if self.files.is_empty() {
            return String::new(); // No output for no changes (like git)
        }

        let mut out = String::new();

        if self.stat_only {
            // --stat mode: compact summary
            for stat in &self.stats {
                let changes = stat.additions + stat.deletions;
                let bar: String = std::iter::repeat_n('+', stat.additions.min(40))
                    .chain(std::iter::repeat_n('-', stat.deletions.min(40)))
                    .collect();
                out.push_str(&format!(" {:<40} | {:>4} {}\n", stat.path, changes, bar));
            }
            out.push_str(&format!(
                " {} file(s) changed, {} insertions(+), {} deletions(-)\n",
                self.files_changed, self.total_additions, self.total_deletions
            ));
            return out;
        }

        for file in &self.files {
            let header = match file.status {
                FileStatus::Added => format!("--- /dev/null\n+++ b/{}\n", file.path),
                FileStatus::Deleted => format!("--- a/{}\n+++ /dev/null\n", file.path),
                FileStatus::Modified => {
                    format!("--- a/{}\n+++ b/{}\n", file.path, file.path)
                }
            };
            out.push_str(&header);

            if file.is_binary {
                out.push_str("Binary files differ\n");
                continue;
            }

            for hunk in &file.hunks {
                out.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
                ));
                if self.word_diff {
                    let annotated = annotate_hunk_word_diff(hunk);
                    for (line, word_segs) in &annotated {
                        if let Some(segs) = word_segs {
                            let prefix = match line.kind {
                                DiffLineKind::Add => '+',
                                DiffLineKind::Remove => '-',
                                _ => ' ',
                            };
                            out.push(prefix);
                            for seg in segs {
                                match seg.kind {
                                    DiffLineKind::Remove => {
                                        out.push_str(&format!("[-{}-]", seg.text));
                                    }
                                    DiffLineKind::Add => {
                                        out.push_str(&format!("{{+{}+}}", seg.text));
                                    }
                                    DiffLineKind::Context => {
                                        out.push_str(&seg.text);
                                    }
                                }
                            }
                            out.push('\n');
                        } else {
                            let prefix = match line.kind {
                                DiffLineKind::Context => ' ',
                                DiffLineKind::Add => '+',
                                DiffLineKind::Remove => '-',
                            };
                            out.push_str(&format!("{}{}\n", prefix, line.content));
                        }
                    }
                } else {
                    for line in &hunk.lines {
                        let prefix = match line.kind {
                            DiffLineKind::Context => ' ',
                            DiffLineKind::Add => '+',
                            DiffLineKind::Remove => '-',
                        };
                        out.push_str(&format!("{}{}\n", prefix, line.content));
                    }
                }
            }
        }

        // Summary line
        out.push_str(&format!(
            "\n{} file(s) changed, {} insertions(+), {} deletions(-)\n",
            self.files_changed, self.total_additions, self.total_deletions
        ));

        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum TagResponse {
    #[serde(rename = "create")]
    Create {
        name: String,
        hash: String,
        annotated: bool,
        signed: bool,
        message: String,
    },
    #[serde(rename = "list")]
    List { tags: Vec<String> },
    #[serde(rename = "delete")]
    Delete { name: String, message: String },
    #[serde(rename = "verify")]
    Verify {
        name: String,
        valid: bool,
        algorithm: String,
        message: String,
    },
}

impl CommandResponse for TagResponse {
    fn command_name(&self) -> &'static str {
        "tag"
    }
    fn human_readable(&self) -> String {
        match self {
            TagResponse::Create { message, .. } => format!("{}\n", message),
            TagResponse::List { tags } => {
                if tags.is_empty() {
                    return String::new();
                }
                tags.iter().map(|t| format!("{}\n", t)).collect()
            }
            TagResponse::Delete { message, .. } => format!("{}\n", message),
            TagResponse::Verify { message, .. } => format!("{}\n", message),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RotateKeyResponse {
    pub objects_rotated: usize,
    pub refs_rotated: usize,
}

impl CommandResponse for RotateKeyResponse {
    fn command_name(&self) -> &'static str {
        "rotate-key"
    }
    fn human_readable(&self) -> String {
        format!(
            "Passphrase rotation complete!\n  {} objects re-encrypted\n  {} refs re-encrypted\n  Old passphrase is no longer valid.\n",
            self.objects_rotated, self.refs_rotated
        )
    }
}

// ============================================================================
// Phase 1.5-1.8 Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct StashEntryInfo {
    pub index: usize,
    pub message: String,
    pub branch: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum StashResponse {
    #[serde(rename = "push")]
    Push { index: usize, message: String },
    #[serde(rename = "pop")]
    Pop { index: usize, message: String },
    #[serde(rename = "apply")]
    Apply { index: usize, message: String },
    #[serde(rename = "list")]
    List { entries: Vec<StashEntryInfo> },
    #[serde(rename = "drop")]
    Drop { index: usize, message: String },
}

impl CommandResponse for StashResponse {
    fn command_name(&self) -> &'static str {
        "stash"
    }
    fn human_readable(&self) -> String {
        match self {
            StashResponse::Push { index, message } => {
                format!(
                    "Saved working directory to stash@{{{}}}: {}",
                    index, message
                )
            }
            StashResponse::Pop { index, message } => {
                format!("Applied and dropped stash@{{{}}}: {}", index, message)
            }
            StashResponse::Apply { index, message } => {
                format!("Applied stash@{{{}}}: {}", index, message)
            }
            StashResponse::List { entries } => {
                if entries.is_empty() {
                    "No stash entries".to_string()
                } else {
                    entries
                        .iter()
                        .map(|e| format!("stash@{{{}}}: {}", e.index, e.message))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            StashResponse::Drop { index, message } => {
                format!("Dropped stash@{{{}}}: {}", index, message)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetResponse {
    pub target: String,
    pub mode: String,
    pub message: String,
}

impl CommandResponse for ResetResponse {
    fn command_name(&self) -> &'static str {
        "reset"
    }
    fn human_readable(&self) -> String {
        format!(
            "HEAD is now at {} ({})\n{}",
            self.target, self.mode, self.message
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevertResponse {
    pub reverted_commit: String,
    pub new_commit: String,
    pub files_changed: usize,
    pub message: String,
}

impl CommandResponse for RevertResponse {
    fn command_name(&self) -> &'static str {
        "revert"
    }
    fn human_readable(&self) -> String {
        format!(
            "Reverted {}\nNew commit: {}\n{} file(s) changed\n{}",
            self.reverted_commit, self.new_commit, self.files_changed, self.message
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CherryPickResponse {
    pub source_commit: String,
    pub new_commit: String,
    pub files_changed: usize,
    pub message: String,
}

impl CommandResponse for CherryPickResponse {
    fn command_name(&self) -> &'static str {
        "cherry-pick"
    }
    fn human_readable(&self) -> String {
        format!(
            "Cherry-picked {}\nNew commit: {}\n{} file(s) changed\n{}",
            self.source_commit, self.new_commit, self.files_changed, self.message
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RebaseResponse {
    pub rebased_commits: usize,
    pub onto: String,
    pub branch: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo: Option<serde_json::Value>,
}

impl CommandResponse for RebaseResponse {
    fn command_name(&self) -> &'static str {
        "rebase"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n", self.message);
        if self.rebased_commits > 0 {
            out.push_str(&format!(
                "Rebased {} commit(s) onto {}\n",
                self.rebased_commits, self.onto
            ));
        }
        if let Some(ref todo) = self.todo {
            out.push_str(&format!(
                "Todo: {}\n",
                serde_json::to_string_pretty(todo).unwrap_or_default()
            ));
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlameLineInfo {
    pub line_number: usize,
    pub content: String,
    pub commit_hash: String,
    pub author: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlameResponse {
    pub file: String,
    pub lines: Vec<BlameLineInfo>,
}

impl CommandResponse for BlameResponse {
    fn command_name(&self) -> &'static str {
        "blame"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("Blame for {}:\n", self.file);
        for line in &self.lines {
            out.push_str(&format!(
                "{} ({} {}) {}\n",
                &line.commit_hash[..8.min(line.commit_hash.len())],
                line.author,
                line.line_number,
                line.content
            ));
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BisectResponse {
    pub action: String,
    pub current: Option<String>,
    pub remaining: usize,
    pub steps: usize,
    pub message: String,
}

impl CommandResponse for BisectResponse {
    fn command_name(&self) -> &'static str {
        "bisect"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n", self.message);
        if let Some(ref commit) = self.current {
            out.push_str(&format!("Current: {}\n", commit));
        }
        if self.remaining > 0 {
            out.push_str(&format!("~{} steps remaining\n", self.steps));
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReflogEntry {
    pub index: usize,
    pub old_hash: String,
    pub new_hash: String,
    pub action: String,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReflogResponse {
    pub ref_name: String,
    pub entries: Vec<ReflogEntry>,
}

impl CommandResponse for ReflogResponse {
    fn command_name(&self) -> &'static str {
        "reflog"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("Reflog for {}:\n", self.ref_name);
        for entry in &self.entries {
            out.push_str(&format!(
                "{}@{{{}}} {} -> {} {}: {}\n",
                self.ref_name,
                entry.index,
                &entry.old_hash[..8.min(entry.old_hash.len())],
                &entry.new_hash[..8.min(entry.new_hash.len())],
                entry.action,
                entry.message
            ));
        }
        out
    }
}

// ============================================================================
// Phase 2 Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchOperationResult {
    pub index: usize,
    pub command: String,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchResponse {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub atomic: bool,
    pub dry_run: bool,
    pub results: Vec<BatchOperationResult>,
}

impl CommandResponse for BatchResponse {
    fn command_name(&self) -> &'static str {
        "batch"
    }
    fn human_readable(&self) -> String {
        format!(
            "Batch complete: {}/{} succeeded, {} failed{}{}",
            self.succeeded,
            self.total,
            self.failed,
            if self.atomic { " (atomic)" } else { "" },
            if self.dry_run { " (dry-run)" } else { "" },
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub action: String,
    pub tx_id: Option<String>,
    pub message: String,
}

impl CommandResponse for TransactionResponse {
    fn command_name(&self) -> &'static str {
        "transaction"
    }
    fn human_readable(&self) -> String {
        if let Some(ref id) = self.tx_id {
            format!(
                "Transaction {}: {} [{}]",
                self.action,
                self.message,
                &id[..8.min(id.len())]
            )
        } else {
            format!("Transaction {}: {}", self.action, self.message)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub hash: String,
    pub short_hash: String,
    pub tree: String,
    pub parent: Option<String>,
    pub author: String,
    pub message: String,
    pub timestamp: i64,
    pub files_added: usize,
}

impl CommandResponse for SnapshotResponse {
    fn command_name(&self) -> &'static str {
        "snapshot"
    }
    fn human_readable(&self) -> String {
        format!(
            "[{}] Snapshot: {}\n  {} file(s) captured\n  Author: {}",
            self.short_hash, self.message, self.files_added, self.author,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line_number: usize,
    pub content: String,
    pub commit: Option<String>,
    pub match_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub match_type: String,
    pub matches: Vec<SearchMatch>,
    pub total: usize,
}

impl CommandResponse for SearchResponse {
    fn command_name(&self) -> &'static str {
        "search"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("Search '{}': {} result(s)\n", self.query, self.total);
        for m in &self.matches {
            match m.match_type.as_str() {
                "content" => {
                    out.push_str(&format!(
                        "  {}:{}: {}\n",
                        m.file,
                        m.line_number,
                        m.content.trim()
                    ));
                }
                "message" => {
                    out.push_str(&format!(
                        "  commit {}: {}\n",
                        m.commit.as_deref().unwrap_or("?"),
                        m.content.trim()
                    ));
                }
                _ => {
                    out.push_str(&format!("  {}\n", m.content.trim()));
                }
            }
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchEvent {
    pub event_type: String,
    pub path: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchResponse {
    pub events_emitted: usize,
    pub message: String,
}

impl CommandResponse for WatchResponse {
    fn command_name(&self) -> &'static str {
        "watch"
    }
    fn human_readable(&self) -> String {
        self.message.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResult {
    pub check: String,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub checks: Vec<VerifyResult>,
    pub objects_checked: usize,
    pub refs_checked: usize,
    pub message: String,
}

impl CommandResponse for VerifyResponse {
    fn command_name(&self) -> &'static str {
        "verify"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n", self.message);
        for check in &self.checks {
            let icon = if check.status == "ok" { "+" } else { "!" };
            out.push_str(&format!("  [{}] {}", icon, check.check));
            if let Some(ref details) = check.details {
                out.push_str(&format!(": {}", details));
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "  {} objects, {} refs checked\n",
            self.objects_checked, self.refs_checked
        ));
        out
    }
}

// ============================================================================
// Phase 3 Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ServeResponse {
    pub message: String,
}

impl CommandResponse for ServeResponse {
    fn command_name(&self) -> &'static str {
        "serve"
    }
    fn human_readable(&self) -> String {
        self.message.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpServeResponse {
    pub transport: String,
    pub message: String,
}

impl CommandResponse for McpServeResponse {
    fn command_name(&self) -> &'static str {
        "mcp-serve"
    }
    fn human_readable(&self) -> String {
        format!("[{}] {}", self.transport, self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwarmResponse {
    pub action: String,
    pub agent_id: Option<String>,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl CommandResponse for SwarmResponse {
    fn command_name(&self) -> &'static str {
        "swarm"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("Swarm {}: {}\n", self.action, self.message);
        if let Some(ref details) = self.details {
            out.push_str(&serde_json::to_string_pretty(details).unwrap_or_default());
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OntologyResponse {
    pub ontology: serde_json::Value,
}

impl CommandResponse for OntologyResponse {
    fn command_name(&self) -> &'static str {
        "ontology"
    }
    fn human_readable(&self) -> String {
        serde_json::to_string_pretty(&self.ontology).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaResponse {
    pub schema: serde_json::Value,
}

impl CommandResponse for SchemaResponse {
    fn command_name(&self) -> &'static str {
        "schema"
    }
    fn human_readable(&self) -> String {
        serde_json::to_string_pretty(&self.schema).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============================================================================
// Phase 4 Response Types (Git Interop)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportGitResponse {
    pub source: String,
    pub objects_imported: u64,
    pub refs_imported: u64,
    pub hash_mapping_count: usize,
    pub message: String,
}

impl CommandResponse for ImportGitResponse {
    fn command_name(&self) -> &'static str {
        "import-git"
    }
    fn human_readable(&self) -> String {
        format!(
            "{}\n  Objects imported: {}\n  Refs imported: {}\n  Hash mappings: {}",
            self.message, self.objects_imported, self.refs_imported, self.hash_mapping_count
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportGitResponse {
    pub destination: String,
    pub objects_exported: u64,
    pub refs_exported: u64,
    pub message: String,
}

impl CommandResponse for ExportGitResponse {
    fn command_name(&self) -> &'static str {
        "export-git"
    }
    fn human_readable(&self) -> String {
        format!(
            "{}\n  Objects exported: {}\n  Refs exported: {}",
            self.message, self.objects_exported, self.refs_exported
        )
    }
}

// ============================================================================
// Phase 5 Response Types (Performance)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GcResponse {
    pub objects_packed: u64,
    pub packs_created: u64,
    pub loose_removed: u64,
    pub bytes_saved: u64,
    pub message: String,
}

impl CommandResponse for GcResponse {
    fn command_name(&self) -> &'static str {
        "gc"
    }
    fn human_readable(&self) -> String {
        format!(
            "{}\n  Objects packed: {}\n  Packs created: {}\n  Loose removed: {}\n  Bytes saved: {}",
            self.message,
            self.objects_packed,
            self.packs_created,
            self.loose_removed,
            self.bytes_saved
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LfsTrackResponse {
    pub patterns: Vec<String>,
    pub message: String,
}

impl CommandResponse for LfsTrackResponse {
    fn command_name(&self) -> &'static str {
        "lfs-track"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n  Tracked patterns:\n", self.message);
        for pat in &self.patterns {
            out.push_str(&format!("    {}\n", pat));
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LfsMigrateResponse {
    pub files_migrated: u64,
    pub bytes_saved: u64,
    pub message: String,
}

impl CommandResponse for LfsMigrateResponse {
    fn command_name(&self) -> &'static str {
        "lfs-migrate"
    }
    fn human_readable(&self) -> String {
        format!(
            "{}\n  Files migrated: {}\n  Bytes saved: {}",
            self.message, self.files_migrated, self.bytes_saved
        )
    }
}

// ============================================================================
// Sandbox Response
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxResponse {
    pub action: String,
    pub name: String,
    pub path: String,
    pub message: String,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
}

impl CommandResponse for SandboxResponse {
    fn command_name(&self) -> &'static str {
        "sandbox"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n", self.message);
        if let Some(ref text) = self.output {
            if !text.is_empty() {
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
        out
    }
}
