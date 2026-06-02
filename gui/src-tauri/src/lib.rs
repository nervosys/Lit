use lit::commands;
use serde_json;

// ─── Status ──────────────────────────────────────────────

#[tauri::command]
fn get_status() -> Result<serde_json::Value, String> {
    let resp = commands::status::execute().map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Branches ────────────────────────────────────────────

#[tauri::command]
fn list_branches() -> Result<serde_json::Value, String> {
    let resp = commands::branch::execute(None, false, false).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_branch(name: String) -> Result<serde_json::Value, String> {
    let resp = commands::branch::execute(Some(name), false, false).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_branch(name: String) -> Result<serde_json::Value, String> {
    let resp = commands::branch::execute(Some(name), true, false).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn checkout_branch(target: String) -> Result<serde_json::Value, String> {
    let resp = commands::checkout::execute(target, false).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Log ─────────────────────────────────────────────────

#[tauri::command]
fn get_log(count: Option<usize>) -> Result<serde_json::Value, String> {
    let n = count.unwrap_or(50);
    let resp = commands::log::execute(n, false).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Diff ────────────────────────────────────────────────

#[tauri::command]
fn get_diff(staged: bool) -> Result<serde_json::Value, String> {
    let resp =
        commands::diff::execute(staged, false, false, None, None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Commit ──────────────────────────────────────────────

#[tauri::command]
fn create_commit(message: String) -> Result<serde_json::Value, String> {
    let resp = commands::commit::execute(message, None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Add / Stage ─────────────────────────────────────────

#[tauri::command]
fn stage_files(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let resp = commands::add::execute(paths).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Stash ───────────────────────────────────────────────

#[tauri::command]
fn stash_save(message: Option<String>) -> Result<serde_json::Value, String> {
    let cmd = lit::StashCommands::Push { message };
    let resp = commands::stash::execute(Some(cmd)).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn stash_list() -> Result<serde_json::Value, String> {
    let resp =
        commands::stash::execute(Some(lit::StashCommands::List)).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn stash_pop() -> Result<serde_json::Value, String> {
    let resp =
        commands::stash::execute(Some(lit::StashCommands::Pop)).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn stash_apply(index: Option<usize>) -> Result<serde_json::Value, String> {
    let resp = commands::stash::execute(Some(lit::StashCommands::Apply { index }))
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn stash_drop(index: Option<usize>) -> Result<serde_json::Value, String> {
    let resp = commands::stash::execute(Some(lit::StashCommands::Drop { index }))
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Undo ────────────────────────────────────────────────

#[tauri::command]
fn undo_list() -> Result<serde_json::Value, String> {
    let resp = commands::undo::execute_list(50).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn undo_undo() -> Result<serde_json::Value, String> {
    let resp = commands::undo::execute_undo(None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
fn undo_redo() -> Result<serde_json::Value, String> {
    let resp = commands::undo::execute_redo(None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Stack ───────────────────────────────────────────────

#[tauri::command]
fn stack_list() -> Result<serde_json::Value, String> {
    let resp = commands::stack::execute_list().map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Search ──────────────────────────────────────────────

#[tauri::command]
fn search_commits(query: String) -> Result<serde_json::Value, String> {
    let resp = commands::search::execute(query, false, None, 50).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Amend ───────────────────────────────────────────────

#[tauri::command]
fn amend_commit(message: Option<String>) -> Result<serde_json::Value, String> {
    let resp = commands::amend::execute(message, None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Reword ──────────────────────────────────────────────

#[tauri::command]
fn reword_commit(message: String) -> Result<serde_json::Value, String> {
    let resp = commands::reword::execute(message, None).map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

// ─── Application entry ──────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            list_branches,
            create_branch,
            delete_branch,
            checkout_branch,
            get_log,
            get_diff,
            create_commit,
            stage_files,
            stash_save,
            stash_list,
            stash_pop,
            stash_apply,
            stash_drop,
            undo_list,
            undo_undo,
            undo_redo,
            stack_list,
            search_commits,
            amend_commit,
            reword_commit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lit GUI");
}
