//! Scrubbed execution environments.
//!
//! # How much this confines, and how you know
//!
//! Confinement comes from [`hv2_sandbox`], which enforces what the host can and
//! reports what it cannot. This module adds the environment scrub and the tree
//! copy, which are hygiene — they keep a build away from dotfiles, credential
//! helpers, the SSH agent and system Git config — and are not a boundary.
//!
//! The division matters because this module used to *be* the whole story: it set
//! a working directory, called `env_clear`, and made no confinement syscall at
//! all, while the README advertised "process isolation with filesystem and
//! network fences" and invited readers to run untrusted code in it.
//!
//! What replaced that is not a promise of isolation either. It is a report:
//! every run fills `controls_enforced` and `controls_unenforced` on the
//! response, the latter carrying the host's own reason for each gap, and
//! [`RunOptions::strict`] refuses the run rather than proceeding with less.
//! On Windows that means resource caps and no network or process isolation; on
//! Linux, namespaces and cgroups unless the user lacks a writable cgroup v2
//! hierarchy, in which case the memory and process-count caps are reported
//! missing rather than assumed present.
//!
//! Filesystem isolation is deliberately not requested — see the comment on the
//! spec in [`execute_run_with`].

use crate::errors::LitError;
use crate::response::SandboxResponse;
use hv2_sandbox::{
    Control, Controls, FilesystemPolicy, NetworkPolicy, ProcessSandbox, Sandbox, SandboxCommand,
    SandboxSpec,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Metadata file placed inside every sandbox root.
const SANDBOX_META: &str = ".sandbox.toml";

/// SECURITY: Validate sandbox name to prevent path traversal.
/// Only allows alphanumeric characters, hyphens, underscores, and dots.
/// Rejects empty names, names starting with a dot, and any path separators.
fn validate_sandbox_name(name: &str) -> Result<(), LitError> {
    if name.is_empty() || name.len() > 128 {
        return Err(LitError::general(
            "sandbox name must be 1-128 characters".to_string(),
        ));
    }
    if name.starts_with('.') {
        return Err(LitError::general(
            "sandbox name must not start with '.'".to_string(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(LitError::general(
            "sandbox name contains invalid characters (allowed: a-z, A-Z, 0-9, -, _, .)"
                .to_string(),
        ));
    }
    Ok(())
}

/// Default sandbox base directory under the repo's `.lit/` folder.
fn sandbox_base(repo_root: &Path) -> PathBuf {
    repo_root.join(".lit").join("sandboxes")
}

/// Resolve the sandbox root for a given name.
fn sandbox_dir(repo_root: &Path, name: &str) -> PathBuf {
    sandbox_base(repo_root).join(name)
}

// ── public entry points ────────────────────────────────────────────

/// Create a new sandbox from the current repo working tree.
pub fn execute_init(name: Option<String>) -> Result<SandboxResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;

    // Use CWD as the source tree when it is inside repo_root.
    // This avoids copying the entire home directory when ~/.lit exists.
    let cwd =
        std::env::current_dir().map_err(|e| LitError::io(format!("cannot determine cwd: {e}")))?;
    let source = if cwd.starts_with(&repo_root) && cwd != repo_root {
        cwd.clone()
    } else {
        repo_root.clone()
    };
    execute_init_from(repo_root, source, name)
}

/// Like [`execute_init`], with the repository root and source tree given.
pub fn execute_init_from(
    repo_root: std::path::PathBuf,
    source: std::path::PathBuf,
    name: Option<String>,
) -> Result<SandboxResponse, LitError> {
    let name =
        name.unwrap_or_else(|| format!("sandbox-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
    validate_sandbox_name(&name)?;

    let sb_dir = sandbox_dir(&repo_root, &name);
    if sb_dir.exists() {
        return Err(LitError::general(format!(
            "sandbox '{}' already exists at {}",
            name,
            sb_dir.display()
        )));
    }
    fs::create_dir_all(&sb_dir)
        .map_err(|e| LitError::io(format!("failed to create sandbox dir: {e}")))?;

    // Copy the source tree (skip .lit/ and sandbox dir itself)
    copy_tree(&source, &sb_dir, &repo_root)?;

    // Write sandbox metadata
    let meta = format!(
        "# Lit sandbox metadata\ncreated = \"{}\"\nsource = \"{}\"\nname = \"{}\"\n",
        chrono::Utc::now().to_rfc3339(),
        source.display(),
        name,
    );
    fs::write(sb_dir.join(SANDBOX_META), &meta)
        .map_err(|e| LitError::io(format!("failed to write sandbox metadata: {e}")))?;

    Ok(SandboxResponse {
        action: "init".into(),
        name: name.clone(),
        path: sb_dir.display().to_string(),
        message: format!("sandbox '{}' created", name),
        output: None,
        exit_code: None,
        controls_enforced: Vec::new(),
        controls_unenforced: Vec::new(),
    })
}

/// Run a command inside an existing sandbox with restricted environment.
/// How strictly [`execute_run_with`] should confine the command.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Refuse to run when this host cannot enforce a requested control, rather
    /// than running with less confinement than was asked for.
    ///
    /// Off by default so the command still works everywhere: Windows enforces
    /// no filesystem or network boundary, and an unprivileged Linux user
    /// usually has no writable cgroup v2 hierarchy. That default is paid for
    /// honestly — a best-effort run reports every control it did not get.
    pub strict: bool,
    /// Let the command reach the network. The default denies it, and denial is
    /// only *enforced* where the host can; read `controls_enforced` to see.
    pub allow_network: bool,
}

pub fn execute_run(name: String, cmd: Vec<String>) -> Result<SandboxResponse, LitError> {
    execute_run_with(name, cmd, RunOptions::default())
}

/// Like [`execute_run`], with explicit confinement options.
pub fn execute_run_with(
    name: String,
    cmd: Vec<String>,
    options: RunOptions,
) -> Result<SandboxResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;
    execute_run_at(repo_root, name, cmd, options)
}

/// Like [`execute_run_with`], against an explicit repository root.
///
/// The confinement backend is the part worth testing, and testing it through
/// the working directory would mean mutating process-global state from a test —
/// so the root is a parameter. This is also what let the sandbox be exercised
/// on Linux CI at all: without it, no test called the run path and the Linux
/// namespace and cgroup code was compiled but never executed.
pub fn execute_run_at(
    repo_root: std::path::PathBuf,
    name: String,
    cmd: Vec<String>,
    options: RunOptions,
) -> Result<SandboxResponse, LitError> {
    validate_sandbox_name(&name)?;
    let sb_dir = sandbox_dir(&repo_root, &name);

    if !sb_dir.join(SANDBOX_META).exists() {
        return Err(LitError::general(format!(
            "sandbox '{}' not found (expected at {})",
            name,
            sb_dir.display()
        )));
    }

    if cmd.is_empty() {
        return Err(LitError::general(String::from(
            "no command specified - use: lit sandbox run <name> -- <command> [args...]",
        )));
    }

    let program = &cmd[0];
    let args = &cmd[1..];

    // The environment scrub is hygiene and stays: it keeps a build away from
    // dotfiles, credential helpers, the SSH agent and system Git config. It is
    // not confinement. The sandbox below is.
    let env = sandboxed_env(&sb_dir);

    let sandbox = ProcessSandbox::new();
    let controls = sandbox.controls();

    let spec = SandboxSpec {
        network: if options.allow_network {
            NetworkPolicy::Host
        } else {
            NetworkPolicy::Denied
        },
        // Deliberately Host. Isolating the filesystem means handing the backend
        // a root to pivot into plus the host paths to mount read-only, and a
        // sandbox directory is a copy of the working tree, not a root
        // filesystem — a workload pivoted into it would find none of its own
        // tools. Choosing that mount set is a design decision, not a default.
        filesystem: FilesystemPolicy::Host,
        isolate_processes: true,
        no_new_privileges: true,
        best_effort: !options.strict,
        ..SandboxSpec::default()
    };

    let mut command = SandboxCommand::new(program).args(args).working_dir(&sb_dir);
    for (key, value) in &env {
        command = command.env(key, value);
    }

    let result = sandbox.run(&command, &spec).map_err(|e| {
        LitError::general(format!(
            "sandbox '{name}' could not run under the requested confinement: {e}. This host enforces {}. Re-run without --strict to proceed with less.",
            describe_controls(&controls)
        ))
    })?;

    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    };

    let code = result.exit_code.unwrap_or(-1);

    Ok(SandboxResponse {
        action: "run".into(),
        name,
        path: sb_dir.display().to_string(),
        // A workload the sandbox killed for exceeding a limit did not merely
        // exit non-zero, and reporting it as an exit code hides the one fact
        // the caller most needs.
        message: match result.killed_by {
            Some(control) => format!(
                "command killed by the sandbox for exceeding its {}",
                control_name(control)
            ),
            None if result.succeeded() => "command completed successfully".into(),
            None => match result.signal {
                Some(sig) => format!("command was terminated by signal {sig}"),
                None => format!("command exited with code {code}"),
            },
        },
        output: Some(combined),
        exit_code: Some(code),
        controls_enforced: controls
            .enforced()
            .iter()
            .map(|c| control_name(*c).to_string())
            .collect(),
        controls_unenforced: unenforced_report(&controls, &result.unenforced),
    })
}

/// List all sandboxes in the current repo.
pub fn execute_list() -> Result<SandboxResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;
    let base = sandbox_base(&repo_root);

    let mut entries = Vec::new();
    if base.exists() {
        for entry in fs::read_dir(&base)
            .map_err(|e| LitError::io(format!("failed to read sandbox dir: {e}")))?
        {
            let entry = entry.map_err(|e| LitError::io(format!("failed to read entry: {e}")))?;
            if entry.path().join(SANDBOX_META).exists() {
                entries.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    entries.sort();

    let message = if entries.is_empty() {
        "no sandboxes".into()
    } else {
        entries.join("\n")
    };

    Ok(SandboxResponse {
        action: "list".into(),
        name: String::new(),
        path: base.display().to_string(),
        message,
        output: None,
        exit_code: None,
        controls_enforced: Vec::new(),
        controls_unenforced: Vec::new(),
    })
}

/// Destroy a sandbox.
pub fn execute_destroy(name: String) -> Result<SandboxResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;
    validate_sandbox_name(&name)?;
    let sb_dir = sandbox_dir(&repo_root, &name);

    if !sb_dir.join(SANDBOX_META).exists() {
        return Err(LitError::general(format!(
            "sandbox '{}' not found (expected at {})",
            name,
            sb_dir.display()
        )));
    }

    fs::remove_dir_all(&sb_dir)
        .map_err(|e| LitError::io(format!("failed to remove sandbox: {e}")))?;

    Ok(SandboxResponse {
        action: "destroy".into(),
        name: name.clone(),
        path: sb_dir.display().to_string(),
        message: format!("sandbox '{}' destroyed", name),
        output: None,
        exit_code: None,
        controls_enforced: Vec::new(),
        controls_unenforced: Vec::new(),
    })
}

// ── helpers ────────────────────────────────────────────────────────

/// Copy the working tree from `src` to `dst`, skipping `.lit/` and hidden VCS dirs.
fn copy_tree(src: &Path, dst: &Path, repo_root: &Path) -> Result<(), LitError> {
    let skip_dirs: std::collections::HashSet<&str> =
        [".lit", ".git", ".hg", "node_modules", "target"]
            .iter()
            .copied()
            .collect();

    for entry in WalkDir::new(src).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        // skip sandbox base itself
        if e.path() == sandbox_base(repo_root) {
            return false;
        }
        // SECURITY: skip symlinks to prevent sandbox escape via symlink traversal
        if e.path_is_symlink() {
            return false;
        }
        if e.file_type().is_dir() && skip_dirs.contains(name.as_ref()) {
            return false;
        }
        true
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| LitError::io(format!("path strip error: {e}")))?;

        if rel.as_os_str().is_empty() {
            continue;
        }

        let target = dst.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| LitError::io(format!("mkdir {}: {e}", target.display())))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| LitError::io(format!("mkdir {}: {e}", parent.display())))?;
            }
            fs::copy(entry.path(), &target).map_err(|e| {
                LitError::io(format!(
                    "copy {} -> {}: {e}",
                    entry.path().display(),
                    target.display()
                ))
            })?;
        }
    }
    Ok(())
}

/// Build a minimal environment map for the sandboxed process.
///
/// Only essential system paths and the sandbox HOME are exposed.
/// Secrets, cloud tokens, user shell config, etc. are stripped.
/// Name a control the way an operator would recognise it.
fn control_name(control: Control) -> &'static str {
    match control {
        Control::Memory => "memory-limit",
        Control::ProcessCount => "process-count-limit",
        Control::CpuTime => "cpu-time-limit",
        Control::WallClock => "wall-clock-deadline",
        Control::NetworkIsolation => "network-isolation",
        Control::FilesystemIsolation => "filesystem-isolation",
        Control::ProcessIsolation => "process-isolation",
        Control::NoNewPrivileges => "no-new-privileges",
    }
}

/// A one-line summary of what this host enforces, for an error message.
fn describe_controls(controls: &Controls) -> String {
    let enforced = controls.enforced();
    if enforced.is_empty() {
        return "nothing".to_string();
    }
    enforced
        .iter()
        .map(|c| control_name(*c))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What was asked for and not granted, each with the reason the host gave.
///
/// The reason matters more than the fact. "no writable cgroup v2 hierarchy:
/// Permission denied" tells an operator to run as root or use a delegated
/// cgroup; "unenforced" tells them nothing.
fn unenforced_report(controls: &Controls, unenforced: &[Control]) -> Vec<String> {
    unenforced
        .iter()
        .map(|c| match controls.reason(*c) {
            Some(reason) => format!("{}: {}", control_name(*c), reason),
            None => control_name(*c).to_string(),
        })
        .collect()
}

fn sandboxed_env(sandbox_root: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Minimal PATH — only system directories
    #[cfg(windows)]
    {
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        env.insert("PATH".into(), format!(r"{sys_root}\System32;{sys_root}"));
        env.insert("SystemRoot".into(), sys_root.clone());
        env.insert("SYSTEMDRIVE".into(), "C:".into());
        env.insert("COMSPEC".into(), format!(r"{sys_root}\System32\cmd.exe"));
    }
    #[cfg(not(windows))]
    {
        env.insert("PATH".into(), "/usr/bin:/bin".into());
    }

    // Set HOME / USERPROFILE to sandbox root so dotfiles can't leak
    let sb = sandbox_root.display().to_string();
    env.insert("HOME".into(), sb.clone());
    #[cfg(windows)]
    env.insert("USERPROFILE".into(), sb.clone());

    // Prevent Git/credential helpers from reaching real config
    env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());

    // Lit-specific: output JSON, disable network
    env.insert("LIT_OUTPUT".into(), "json".into());
    env.insert("LIT_AIRGAPPED".into(), "1".into());

    // Timezone (informational)
    if let Ok(tz) = std::env::var("TZ") {
        env.insert("TZ".into(), tz);
    }

    // TEMP dirs inside sandbox
    let tmp = sandbox_root.join("tmp");
    let _ = fs::create_dir_all(&tmp);
    let tmp_str = tmp.display().to_string();
    env.insert("TMPDIR".into(), tmp_str.clone());
    env.insert("TEMP".into(), tmp_str.clone());
    env.insert("TMP".into(), tmp_str);

    env
}
