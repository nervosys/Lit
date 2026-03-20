use crate::errors::LitError;
use crate::response::SandboxResponse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    })
}

/// Run a command inside an existing sandbox with restricted environment.
pub fn execute_run(name: String, cmd: Vec<String>) -> Result<SandboxResponse, LitError> {
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

    if cmd.is_empty() {
        return Err(LitError::general(String::from(
            "no command specified - use: lit sandbox run <name> -- <command> [args...]",
        )));
    }

    let program = &cmd[0];
    let args = &cmd[1..];

    // Build a minimal, sandboxed environment.
    let env = sandboxed_env(&sb_dir);

    let result = Command::new(program)
        .args(args)
        .current_dir(&sb_dir)
        .env_clear()
        .envs(&env)
        .output()
        .map_err(|e| LitError::io(format!("failed to spawn command: {e}")))?;

    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    };

    let code = result.status.code().unwrap_or(-1);

    Ok(SandboxResponse {
        action: "run".into(),
        name,
        path: sb_dir.display().to_string(),
        message: if result.status.success() {
            "command completed successfully".into()
        } else {
            format!("command exited with code {code}")
        },
        output: Some(combined),
        exit_code: Some(code),
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
