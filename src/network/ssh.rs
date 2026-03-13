/// SSH transport stub for remote Lit repositories
///
/// Will provide SSH-based transport using the SSH protocol.
/// Currently returns an error indicating the feature is not yet implemented.
///
/// Fetch objects from a remote repository over SSH
pub fn fetch(_url: &str, _refs: &[&str]) -> Result<Vec<u8>, String> {
    Err("SSH transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Push objects to a remote repository over SSH
pub fn push(_url: &str, _refs: &[(&str, &str)]) -> Result<(), String> {
    Err("SSH transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Discover refs advertised by a remote SSH repository
pub fn ls_remote(_url: &str) -> Result<Vec<(String, String)>, String> {
    Err("SSH transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Check whether a URL uses the SSH transport
pub fn is_ssh_url(url: &str) -> bool {
    url.starts_with("ssh://") || (url.contains('@') && url.contains(':') && !url.contains("://"))
}
