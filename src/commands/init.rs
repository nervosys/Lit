use crate::response::InitResponse;
use std::fs;
use std::path::PathBuf;

pub fn execute(bare: bool, path: Option<String>) -> Result<InitResponse, String> {
    let repo_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    };

    // Check if already a repository
    if repo_path.join(".lit").exists() {
        return Err(format!(
            "Repository already exists at {}",
            repo_path.display()
        ));
    }

    // Create .lit directory
    let lit_dir = repo_path.join(".lit");
    fs::create_dir_all(&lit_dir).map_err(|e| format!("Failed to create .lit directory: {}", e))?;

    // Create subdirectories
    fs::create_dir_all(lit_dir.join("objects"))
        .map_err(|e| format!("Failed to create objects directory: {}", e))?;

    fs::create_dir_all(lit_dir.join("refs").join("heads"))
        .map_err(|e| format!("Failed to create refs/heads directory: {}", e))?;

    fs::create_dir_all(lit_dir.join("refs").join("tags"))
        .map_err(|e| format!("Failed to create refs/tags directory: {}", e))?;

    fs::create_dir_all(lit_dir.join("refs").join("remotes"))
        .map_err(|e| format!("Failed to create refs/remotes directory: {}", e))?;

    // Create HEAD
    if !bare {
        fs::write(lit_dir.join("HEAD"), "ref: refs/heads/main\n")
            .map_err(|e| format!("Failed to create HEAD: {}", e))?;
    }

    // Create config file
    let config_content = if bare {
        "[core]\n    bare = true\n"
    } else {
        "[core]\n    bare = false\n"
    };

    fs::write(lit_dir.join("config"), config_content)
        .map_err(|e| format!("Failed to create config: {}", e))?;

    // Create description
    fs::write(lit_dir.join("description"), "Unnamed Lit repository.\n")
        .map_err(|e| format!("Failed to create description: {}", e))?;

    // Create empty index
    use crate::storage::Index;
    let index = Index::new();
    index.save(&repo_path)?;

    Ok(InitResponse {
        path: lit_dir.display().to_string(),
        bare,
    })
}
