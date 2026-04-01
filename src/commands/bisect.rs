use crate::core::{find_repo_root, read_head, Object, ObjectHash};
use crate::response::BisectResponse;
use crate::storage::ObjectStore;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BisectState {
    good: Vec<String>,
    bad: Vec<String>,
    current: Option<String>,
    remaining: Vec<String>,
    steps: usize,
}

pub fn execute(
    command: Option<crate::BisectCommands>,
) -> Result<BisectResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    match command {
        Some(crate::BisectCommands::Start) => bisect_start(&repo_root),
        Some(crate::BisectCommands::Good { commit }) => bisect_mark(&repo_root, &commit, true),
        Some(crate::BisectCommands::Bad { commit }) => bisect_mark(&repo_root, &commit, false),
        Some(crate::BisectCommands::Reset) => bisect_reset(&repo_root),
        None => {
            // Show current bisect status
            let state = load_bisect_state(&repo_root)?;
            Ok(BisectResponse {
                action: "status".to_string(),
                current: state.current,
                remaining: state.remaining.len(),
                steps: state.steps,
                message: format!("Bisecting: {} commits left to test", state.remaining.len()),
            })
        }
    }
}

fn bisect_start(repo_root: &std::path::Path) -> Result<BisectResponse, crate::errors::LitError> {
    let state = BisectState {
        good: Vec::new(),
        bad: Vec::new(),
        current: None,
        remaining: Vec::new(),
        steps: 0,
    };

    save_bisect_state(repo_root, &state)?;

    Ok(BisectResponse {
        action: "start".to_string(),
        current: None,
        remaining: 0,
        steps: 0,
        message: "Bisect started. Mark commits as good or bad.".to_string(),
    })
}

fn bisect_mark(
    repo_root: &std::path::Path,
    commit: &str,
    is_good: bool,
) -> Result<BisectResponse, crate::errors::LitError> {
    let mut state = load_bisect_state(repo_root)?;
    let store = ObjectStore::new(repo_root);

    let hash = if commit == "HEAD" {
        read_head(repo_root)?
    } else {
        commit.to_string()
    };

    if is_good {
        state.good.push(hash.clone());
    } else {
        state.bad.push(hash.clone());
    }

    // If we have both good and bad, compute midpoint
    if !state.good.is_empty() && !state.bad.is_empty() {
        let bad_hash = state.bad.last().unwrap().clone();
        let good_hash = state.good.last().unwrap().clone();

        // Collect commits between bad and good
        let commits = collect_commits_between(&store, &bad_hash, &good_hash)?;
        state.remaining = commits;

        if state.remaining.is_empty() {
            // Found the first bad commit
            save_bisect_state(repo_root, &state)?;
            return Ok(BisectResponse {
                action: if is_good { "good" } else { "bad" }.to_string(),
                current: Some(bad_hash[..16.min(bad_hash.len())].to_string()),
                remaining: 0,
                steps: state.steps,
                message: format!("First bad commit: {}", &bad_hash[..16.min(bad_hash.len())]),
            });
        }

        // Pick midpoint
        let mid = state.remaining.len() / 2;
        state.current = Some(state.remaining[mid].clone());
        state.steps += 1;

        let est_steps = (state.remaining.len() as f64).log2().ceil() as usize;

        save_bisect_state(repo_root, &state)?;

        let current_short = state
            .current
            .as_ref()
            .map(|c| c[..16.min(c.len())].to_string());

        Ok(BisectResponse {
            action: if is_good { "good" } else { "bad" }.to_string(),
            current: current_short,
            remaining: state.remaining.len(),
            steps: est_steps,
            message: format!(
                "Bisecting: {} commits left to test (~{} steps)",
                state.remaining.len(),
                est_steps
            ),
        })
    } else {
        save_bisect_state(repo_root, &state)?;
        Ok(BisectResponse {
            action: if is_good { "good" } else { "bad" }.to_string(),
            current: None,
            remaining: 0,
            steps: 0,
            message: format!(
                "Marked {} as {}. Need both good and bad commits to start bisecting.",
                &hash[..16.min(hash.len())],
                if is_good { "good" } else { "bad" }
            ),
        })
    }
}

fn bisect_reset(repo_root: &std::path::Path) -> Result<BisectResponse, crate::errors::LitError> {
    let bisect_path = repo_root.join(".lit").join("bisect.json");
    if bisect_path.exists() {
        fs::remove_file(&bisect_path)
            .map_err(|e| format!("Failed to remove bisect state: {}", e))?;
    }

    Ok(BisectResponse {
        action: "reset".to_string(),
        current: None,
        remaining: 0,
        steps: 0,
        message: "Bisect reset".to_string(),
    })
}

fn collect_commits_between(
    store: &ObjectStore,
    bad: &str,
    good: &str,
) -> Result<Vec<String>, crate::errors::LitError> {
    let mut commits = Vec::new();
    let mut current = bad.to_string();

    loop {
        if current == good {
            break;
        }

        let hash = ObjectHash::from_hex(current.clone());
        let commit = match store.read(&hash) {
            Ok(Object::Commit(c)) => c,
            _ => break,
        };

        commits.push(current);

        match commit.parents.first() {
            Some(p) => current = p.to_string(),
            None => break,
        }
    }

    Ok(commits)
}

fn load_bisect_state(repo_root: &std::path::Path) -> Result<BisectState, crate::errors::LitError> {
    let path = repo_root.join(".lit").join("bisect.json");
    if !path.exists() {
        return Err("No bisect in progress. Run `lit bisect start` first.".into());
    }
    let data =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read bisect state: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse bisect state: {}", e).into())
}

fn save_bisect_state(
    repo_root: &std::path::Path,
    state: &BisectState,
) -> Result<(), crate::errors::LitError> {
    let path = repo_root.join(".lit").join("bisect.json");
    let data = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize bisect state: {}", e))?;
    fs::write(&path, data).map_err(|e| format!("Failed to write bisect state: {}", e).into())
}
