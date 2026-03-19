//! HTTPS transport for remote Lit repositories
//!
//! Communicates with a remote `lit serve` instance over HTTP/HTTPS.
//! Uses the transport API endpoints for object and ref transfer.

use crate::core::{Object, ObjectHash};
use crate::network::transport::RemoteRef;
use crate::storage::ObjectStore;

/// Check whether a URL uses the HTTPS transport
pub fn is_https_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Create a ureq agent (reusable HTTP client)
fn agent() -> ureq::Agent {
    ureq::Agent::new()
}

/// Build an HTTP request with optional bearer token auth
fn get(url: &str, token: Option<&str>) -> ureq::Request {
    let req = agent().get(url);
    if let Some(t) = token {
        req.set("Authorization", &format!("Bearer {}", t))
    } else {
        req
    }
}

fn post(url: &str, token: Option<&str>) -> ureq::Request {
    let req = agent().post(url);
    if let Some(t) = token {
        req.set("Authorization", &format!("Bearer {}", t))
    } else {
        req
    }
}

fn put(url: &str, token: Option<&str>) -> ureq::Request {
    let req = agent().put(url);
    if let Some(t) = token {
        req.set("Authorization", &format!("Bearer {}", t))
    } else {
        req
    }
}

/// Parse a JSON response body
fn read_json(resp: ureq::Response) -> Result<serde_json::Value, String> {
    resp.into_json::<serde_json::Value>()
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Handle HTTP errors
fn check_response(resp: Result<ureq::Response, ureq::Error>) -> Result<ureq::Response, String> {
    match resp {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            // Try to extract error message from JSON
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(msg) = v
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                {
                    return Err(format!("HTTP {}: {}", code, msg));
                }
            }
            Err(format!("HTTP {}: {}", code, body))
        }
        Err(ureq::Error::Transport(t)) => Err(format!("Connection error: {}", t)),
    }
}

/// List refs from a remote server
pub fn list_refs_http(
    base_url: &str,
    kind: &str,
    token: Option<&str>,
) -> Result<Vec<RemoteRef>, String> {
    let url = format!("{}/api/v1/transport/refs?kind={}", base_url, kind);
    let resp = check_response(get(&url, token).call())?;
    let json = read_json(resp)?;

    let refs = json
        .get("refs")
        .and_then(|v| v.as_array())
        .ok_or("Invalid refs response")?;

    let mut result = Vec::new();
    for r in refs {
        let kind = r.get("kind").and_then(|v| v.as_str()).unwrap_or("heads");
        let name = r
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing ref name")?;
        let hash = r
            .get("hash")
            .and_then(|v| v.as_str())
            .ok_or("Missing ref hash")?;
        result.push(RemoteRef {
            kind: kind.to_string(),
            name: name.to_string(),
            hash: hash.to_string(),
        });
    }
    Ok(result)
}

/// Read a branch ref from a remote server
pub fn read_ref_http(base_url: &str, branch: &str, token: Option<&str>) -> Result<String, String> {
    let url = format!("{}/api/v1/transport/refs/heads/{}", base_url, branch);
    let resp = check_response(get(&url, token).call())?;
    let json = read_json(resp)?;
    json.get("hash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing hash in response".to_string())
}

/// Read HEAD from a remote server
pub fn read_head_http(base_url: &str, token: Option<&str>) -> Result<String, String> {
    let url = format!("{}/api/v1/transport/head", base_url);
    let resp = check_response(get(&url, token).call())?;
    let json = read_json(resp)?;
    json.get("head")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing head in response".to_string())
}

/// Update a branch ref on a remote server
pub fn update_ref_http(
    base_url: &str,
    branch: &str,
    hash: &str,
    force: bool,
    token: Option<&str>,
) -> Result<(), String> {
    let url = format!("{}/api/v1/transport/refs/heads/{}", base_url, branch);
    let body = serde_json::json!({"hash": hash, "force": force});
    check_response(put(&url, token).send_json(body))?;
    Ok(())
}

/// Negotiate which objects are needed (server-side graph walk)
pub fn negotiate_http(
    base_url: &str,
    wants: &[String],
    haves: &[String],
    token: Option<&str>,
) -> Result<Vec<ObjectHash>, String> {
    let url = format!("{}/api/v1/transport/negotiate", base_url);
    let body = serde_json::json!({"wants": wants, "haves": haves});
    let resp = check_response(post(&url, token).send_json(body))?;
    let json = read_json(resp)?;

    let needed = json
        .get("needed")
        .and_then(|v| v.as_array())
        .ok_or("Invalid negotiate response")?;

    Ok(needed
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| ObjectHash::from_hex(s.to_string()))
        .collect())
}

/// Download objects from a remote server into a local store
pub fn download_objects_http(
    base_url: &str,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
    token: Option<&str>,
) -> Result<usize, String> {
    let mut count = 0;
    // Download in batches to avoid excessive individual requests
    for chunk in hashes.chunks(50) {
        for hash in chunk {
            if local_store.exists(hash) {
                continue;
            }
            let url = format!("{}/api/v1/transport/objects/{}", base_url, hash.as_str());
            let resp = check_response(get(&url, token).call())?;
            let json = read_json(resp)?;

            let b64_data = json
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or("Missing object data")?;

            let compressed = base64_decode(b64_data)?;

            use std::io::Read as _;
            let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
            let mut raw = Vec::new();
            decoder
                .read_to_end(&mut raw)
                .map_err(|e| format!("Decompress error: {}", e))?;

            let obj = Object::from_bytes(&raw)?;
            local_store.write(&obj)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Upload objects from a local store to a remote server
pub fn upload_objects_http(
    base_url: &str,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
    token: Option<&str>,
) -> Result<usize, String> {
    let url = format!("{}/api/v1/transport/objects", base_url);

    // Upload in batches of 50
    let mut total = 0;
    for chunk in hashes.chunks(50) {
        let mut objects_json = Vec::new();
        for hash in chunk {
            let obj = local_store.read(hash)?;
            let data = obj.to_bytes();
            use std::io::Write as _;
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder
                .write_all(&data)
                .map_err(|e| format!("Compress error: {}", e))?;
            let compressed = encoder
                .finish()
                .map_err(|e| format!("Compress error: {}", e))?;
            let b64 = base64_encode(&compressed);
            objects_json.push(serde_json::json!({"hash": hash.as_str(), "data": b64}));
        }

        let body = serde_json::json!({"objects": objects_json});
        let resp = check_response(post(&url, token).send_json(body))?;
        let json = read_json(resp)?;
        total += json.get("written").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    }
    Ok(total)
}

// ── Base64 helpers ──

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("Invalid base64 character: {}", c as char)),
        }
    }
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            return Err("Invalid base64 length".to_string());
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = val(chunk[2])?;
        let d = val(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}
