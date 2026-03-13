/// HTTPS transport stub for remote Lit repositories
///
/// Will provide Smart HTTP protocol support similar to Git's smart HTTP.
/// Currently returns an error indicating the feature is not yet implemented.
///
/// Fetch objects from a remote repository over HTTPS
pub fn fetch(_url: &str, _refs: &[&str]) -> Result<Vec<u8>, String> {
    Err("HTTPS transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Push objects to a remote repository over HTTPS
pub fn push(_url: &str, _refs: &[(&str, &str)]) -> Result<(), String> {
    Err("HTTPS transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Discover refs advertised by a remote HTTPS repository
pub fn ls_remote(_url: &str) -> Result<Vec<(String, String)>, String> {
    Err("HTTPS transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Check whether a URL uses the HTTPS transport
pub fn is_https_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
