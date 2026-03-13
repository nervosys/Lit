/// Native Lit protocol transport (`lit://`) for remote Lit repositories
///
/// The `lit://` protocol is Lit's native encrypted transport, designed for
/// secure object transfer with post-quantum cryptography. It uses TLS 1.3
/// with ML-KEM key exchange and provides authenticated, encrypted channels
/// between Lit repository endpoints.
///
/// Currently returns an error indicating the feature is not yet implemented.
///
/// Fetch objects from a remote repository over the lit:// protocol
pub fn fetch(_url: &str, _refs: &[&str]) -> Result<Vec<u8>, String> {
    Err("lit:// transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Push objects to a remote repository over the lit:// protocol
pub fn push(_url: &str, _refs: &[(&str, &str)]) -> Result<(), String> {
    Err("lit:// transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Discover refs advertised by a remote lit:// repository
pub fn ls_remote(_url: &str) -> Result<Vec<(String, String)>, String> {
    Err("lit:// transport is not yet implemented. Use file:// or local path remotes.".to_string())
}

/// Check whether a URL uses the lit:// transport
pub fn is_lit_url(url: &str) -> bool {
    url.starts_with("lit://")
}

/// Default port for the lit:// protocol
pub const DEFAULT_PORT: u16 = 9418;
