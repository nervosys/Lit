//! Native Lit protocol transport (`lit://`) for remote Lit repositories
//!
//! The `lit://` protocol is Lit's native transport using TCP connections
//! with newline-delimited JSON framing. The server side is provided by
//! `lit serve --daemon`, which accepts TCP connections on the configured
//! port (default 9418) and processes the same JSON API as the stdio and
//! HTTP modes.

use crate::core::{Object, ObjectHash};
use crate::network::transport::RemoteRef;
use crate::storage::ObjectStore;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::TcpStream;

/// Default port for the lit:// protocol
pub const DEFAULT_PORT: u16 = 9418;

/// Check whether a URL uses the lit:// transport
pub fn is_lit_url(url: &str) -> bool {
    url.starts_with("lit://")
}

/// Parsed lit:// URL components
#[derive(Debug, Clone)]
pub struct LitUrl {
    pub host: String,
    pub port: u16,
    pub path: String,
}

/// Parse a `lit://host[:port]/path` URL into its components
pub fn parse_lit_url(url: &str) -> Result<LitUrl, String> {
    let rest = url
        .strip_prefix("lit://")
        .ok_or_else(|| format!("Not a lit:// URL: {}", url))?;

    let (hostport, path) = rest
        .split_once('/')
        .ok_or_else(|| format!("Invalid lit:// URL (missing path): {}", url))?;

    let (host, port) = if let Some((h, p)) = hostport.split_once(':') {
        let port_num = p
            .parse::<u16>()
            .map_err(|_| format!("Invalid port in lit:// URL: {}", p))?;
        (h.to_string(), port_num)
    } else {
        (hostport.to_string(), DEFAULT_PORT)
    };

    if host.is_empty() {
        return Err(format!("Empty host in lit:// URL: {}", url));
    }

    Ok(LitUrl {
        host,
        port,
        path: format!("/{}", path),
    })
}

/// A TCP connection to a remote `lit serve --daemon` instance
pub struct LitConnection {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl LitConnection {
    /// Connect to a remote lit:// daemon
    pub fn open(parsed: &LitUrl) -> Result<Self, String> {
        let addr = format!("{}:{}", parsed.host, parsed.port);
        let stream = TcpStream::connect(&addr)
            .map_err(|e| format!("Failed to connect to lit://{}: {}", addr, e))?;

        let reader_stream = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone TCP stream: {}", e))?;

        Ok(LitConnection {
            reader: BufReader::new(reader_stream),
            writer: BufWriter::new(stream),
        })
    }

    /// Connect to a local daemon (for testing)
    pub fn open_local(port: u16) -> Result<Self, String> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr)
            .map_err(|e| format!("Failed to connect to lit daemon at {}: {}", addr, e))?;

        let reader_stream = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone TCP stream: {}", e))?;

        Ok(LitConnection {
            reader: BufReader::new(reader_stream),
            writer: BufWriter::new(stream),
        })
    }

    /// Send a request and read the response
    fn request(
        &mut self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<(u16, serde_json::Value), String> {
        let req = serde_json::json!({
            "method": method,
            "path": path,
            "body": body,
        });
        writeln!(self.writer, "{}", req)
            .map_err(|e| format!("Failed to write to lit:// connection: {}", e))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush lit:// connection: {}", e))?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read from lit:// connection: {}", e))?;

        if line.is_empty() {
            return Err("lit:// connection closed unexpectedly".to_string());
        }

        let resp: serde_json::Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("Invalid JSON from lit:// connection: {}", e))?;

        let status = resp.get("status").and_then(|v| v.as_u64()).unwrap_or(500) as u16;

        let body_str = resp.get("body").and_then(|v| v.as_str()).unwrap_or("{}");

        let body_json: serde_json::Value =
            serde_json::from_str(body_str).unwrap_or_else(|_| serde_json::json!({"raw": body_str}));

        Ok((status, body_json))
    }
}

/// Check response status and extract error messages
fn check_status(status: u16, body: &serde_json::Value) -> Result<(), String> {
    if status >= 400 {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| body.get("raw").and_then(|v| v.as_str()))
            .unwrap_or("Unknown error");
        Err(format!("lit:// transport error ({}): {}", status, msg))
    } else {
        Ok(())
    }
}

/// List refs from a remote server via lit:// connection
pub fn list_refs_lit(conn: &mut LitConnection, kind: &str) -> Result<Vec<RemoteRef>, String> {
    let path = format!("/api/v1/transport/refs?kind={}", kind);
    let (status, body) = conn.request("GET", &path, "")?;
    check_status(status, &body)?;

    let refs = body
        .get("refs")
        .and_then(|v| v.as_array())
        .ok_or("Invalid refs response from lit://")?;

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

/// Read a branch ref from a remote server via lit:// connection
pub fn read_ref_lit(conn: &mut LitConnection, branch: &str) -> Result<String, String> {
    let path = format!("/api/v1/transport/refs/heads/{}", branch);
    let (status, body) = conn.request("GET", &path, "")?;
    check_status(status, &body)?;
    body.get("hash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing hash in lit:// response".to_string())
}

/// Read HEAD from a remote server via lit:// connection
pub fn read_head_lit(conn: &mut LitConnection) -> Result<String, String> {
    let (status, body) = conn.request("GET", "/api/v1/transport/head", "")?;
    check_status(status, &body)?;
    body.get("head")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing head in lit:// response".to_string())
}

/// Update a branch ref on a remote server via lit:// connection
pub fn update_ref_lit(
    conn: &mut LitConnection,
    branch: &str,
    hash: &str,
    force: bool,
) -> Result<(), String> {
    let path = format!("/api/v1/transport/refs/heads/{}", branch);
    let body = serde_json::json!({"hash": hash, "force": force}).to_string();
    let (status, resp) = conn.request("PUT", &path, &body)?;
    check_status(status, &resp)
}

/// Negotiate which objects are needed via lit:// connection
pub fn negotiate_lit(
    conn: &mut LitConnection,
    wants: &[String],
    haves: &[String],
) -> Result<Vec<ObjectHash>, String> {
    let body = serde_json::json!({"wants": wants, "haves": haves}).to_string();
    let (status, resp) = conn.request("POST", "/api/v1/transport/negotiate", &body)?;
    check_status(status, &resp)?;

    let needed = resp
        .get("needed")
        .and_then(|v| v.as_array())
        .ok_or("Invalid negotiate response from lit://")?;

    Ok(needed
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| ObjectHash::from_hex(s.to_string()))
        .collect())
}

/// Download objects from a remote server via lit:// connection
pub fn download_objects_lit(
    conn: &mut LitConnection,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
) -> Result<usize, String> {
    let mut count = 0;
    for hash in hashes {
        if local_store.exists(hash) {
            continue;
        }
        let path = format!("/api/v1/transport/objects/{}", hash.as_str());
        let (status, body) = conn.request("GET", &path, "")?;
        check_status(status, &body)?;

        let b64_data = body
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or("Missing object data in lit:// response")?;

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
    Ok(count)
}

/// Upload objects from a local store to a remote server via lit:// connection
pub fn upload_objects_lit(
    conn: &mut LitConnection,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
) -> Result<usize, String> {
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

        let body = serde_json::json!({"objects": objects_json}).to_string();
        let (status, resp) = conn.request("POST", "/api/v1/transport/objects", &body)?;
        check_status(status, &resp)?;
        total += resp.get("written").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
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
