use crate::errors::LitError;
use crate::response::CommandResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AiResponse {
    pub action: String,
    pub generated: String,
    pub message: String,
    pub model: Option<String>,
}

impl CommandResponse for AiResponse {
    fn command_name(&self) -> &'static str {
        "ai"
    }
    fn human_readable(&self) -> String {
        match self.action.as_str() {
            "commit-message" => format!("Generated commit message:\n  {}\n", self.generated),
            "branch-name" => format!("Suggested branch name: {}\n", self.generated),
            "pr-description" => format!("Generated PR description:\n{}\n", self.generated),
            _ => format!("{}: {}\n", self.action, self.generated),
        }
    }
}

/// AI configuration stored in lit config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub endpoint: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key_env: "LIT_AI_API_KEY".to_string(),
            endpoint: None,
        }
    }
}

/// Generate a commit message from the current staged diff
pub fn execute_commit_message(context: Option<String>) -> Result<AiResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;

    // Get the current diff to use as context
    let diff_result = crate::commands::diff::execute(true, false, false, None, None)?;
    let diff_text = serde_json::to_string(&diff_result).unwrap_or_default();

    if diff_text.is_empty() || diff_text == "{}" || diff_text.contains("\"files\":[]") {
        return Err(LitError::general(
            "No staged changes to generate commit message from",
        ));
    }

    // Try to call AI API (requires configured API key)
    let config = load_ai_config(&repo_root);
    let api_key = std::env::var(&config.api_key_env).ok();

    let generated = if let Some(key) = api_key {
        call_ai_api(
            &config,
            &key,
            &format!(
                "Generate a concise, conventional commit message for the following diff. \
                 Use imperative mood. Keep it under 72 characters for the subject line. \
                 {} \n\nDiff:\n{}",
                context.as_deref().unwrap_or(""),
                &diff_text[..diff_text.len().min(4000)]
            ),
        )?
    } else {
        // Fallback: generate a basic message from file names
        generate_fallback_commit_message(&diff_text)
    };

    Ok(AiResponse {
        action: "commit-message".to_string(),
        generated,
        message: "Commit message generated".to_string(),
        model: Some(config.model),
    })
}

/// Generate a branch name from a description
pub fn execute_branch_name(description: String) -> Result<AiResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;
    let config = load_ai_config(&repo_root);
    let api_key = std::env::var(&config.api_key_env).ok();

    let generated = if let Some(key) = api_key {
        call_ai_api(
            &config,
            &key,
            &format!(
                "Generate a short, kebab-case git branch name (max 50 chars) for: {}",
                description
            ),
        )?
    } else {
        // Fallback: simple kebab-case conversion
        description
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
            .trim_matches('-')
            .to_string()
    };

    Ok(AiResponse {
        action: "branch-name".to_string(),
        generated,
        message: "Branch name generated".to_string(),
        model: Some(config.model),
    })
}

/// Generate a PR description from branch diff
pub fn execute_pr_description(
    head: Option<String>,
    base: Option<String>,
) -> Result<AiResponse, LitError> {
    let repo_root = crate::core::find_repo_root()?;
    let config = load_ai_config(&repo_root);
    let api_key = std::env::var(&config.api_key_env).ok();

    let head_ref = head.unwrap_or_else(|| {
        crate::core::get_current_branch(&repo_root).unwrap_or_else(|_| "HEAD".to_string())
    });
    let base_ref = base.unwrap_or_else(|| "main".to_string());

    // Get diff between branches
    let diff_result = crate::commands::diff::execute(
        false,
        false,
        false,
        Some(base_ref.clone()),
        Some(head_ref.clone()),
    )?;
    let diff_text = serde_json::to_string(&diff_result).unwrap_or_default();

    let generated = if let Some(key) = api_key {
        call_ai_api(
            &config,
            &key,
            &format!(
                "Generate a pull request description for merging '{}' into '{}'. \
                 Include: summary, changes made, testing notes. Use markdown formatting.\n\n\
                 Diff:\n{}",
                head_ref,
                base_ref,
                &diff_text[..diff_text.len().min(4000)]
            ),
        )?
    } else {
        format!(
            "## Summary\n\nMerge `{}` into `{}`\n\n## Changes\n\n- See diff for details\n",
            head_ref, base_ref
        )
    };

    Ok(AiResponse {
        action: "pr-description".to_string(),
        generated,
        message: "PR description generated".to_string(),
        model: Some(config.model),
    })
}

fn load_ai_config(repo_root: &std::path::Path) -> AiConfig {
    let config_path = repo_root.join(".lit").join("ai.json");
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => AiConfig::default(),
        }
    } else {
        AiConfig::default()
    }
}

fn call_ai_api(config: &AiConfig, api_key: &str, prompt: &str) -> Result<String, LitError> {
    let endpoint = config
        .endpoint
        .as_deref()
        .unwrap_or(match config.provider.as_str() {
            "openai" => "https://api.openai.com/v1/chat/completions",
            "anthropic" => "https://api.anthropic.com/v1/messages",
            _ => "https://api.openai.com/v1/chat/completions",
        });

    let body = match config.provider.as_str() {
        "anthropic" => serde_json::json!({
            "model": config.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        }),
        _ => serde_json::json!({
            "model": config.model,
            "messages": [
                {"role": "system", "content": "You are a helpful assistant for version control operations. Be concise."},
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 1024,
            "temperature": 0.3
        }),
    };

    let auth_header = match config.provider.as_str() {
        "anthropic" => ("x-api-key", api_key.to_string()),
        _ => ("Authorization", format!("Bearer {}", api_key)),
    };

    let response = ureq::post(endpoint)
        .set(auth_header.0, &auth_header.1)
        .set("Content-Type", "application/json")
        .send_string(
            &serde_json::to_string(&body)
                .map_err(|e| LitError::general(format!("Failed to serialize request: {}", e)))?,
        )
        .map_err(|e| LitError::general(format!("AI API request failed: {}", e)))?;

    let response_body: serde_json::Value = response
        .into_json()
        .map_err(|e| LitError::general(format!("Failed to parse AI response: {}", e)))?;

    // Extract text from OpenAI-style or Anthropic-style response
    let text = response_body["choices"][0]["message"]["content"]
        .as_str()
        .or_else(|| response_body["content"][0]["text"].as_str())
        .unwrap_or("Failed to generate text")
        .trim()
        .to_string();

    Ok(text)
}

fn generate_fallback_commit_message(diff_text: &str) -> String {
    // Parse file names from diff output
    let files: Vec<&str> = diff_text
        .lines()
        .filter(|l| l.contains("\"path\"") || l.contains("\"file\""))
        .take(5)
        .collect();

    if files.is_empty() {
        "Update files".to_string()
    } else if files.len() == 1 {
        format!("Update {}", files[0].trim().replace(['"', ','], ""))
    } else {
        format!("Update {} files", files.len())
    }
}
