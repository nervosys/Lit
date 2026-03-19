//! SSH transport for remote Lit repositories
//!
//! Communicates with a remote `lit serve --stdio` instance over SSH.
//! The SSH client spawns the system `ssh` command and communicates
//! via newline-delimited JSON over stdin/stdout pipes.

use crate::core::{Object, ObjectHash};
use crate::network::transport::RemoteRef;
use crate::storage::ObjectStore;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};

/// Check whether a URL uses the SSH transport
pub fn is_ssh_url(url: &str) -> bool {
    url.starts_with("ssh://") || (url.contains('@') && url.contains(':') && !url.contains("://"))
}

/// Parsed SSH URL components
#[derive(Debug, Clone)]
pub struct SshUrl {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

/// Parse an SSH URL into its components.
///
/// Supports two formats:
/// - `ssh://[user@]host[:port]/path`
/// - `user@host:path` (SCP-style)
pub fn parse_ssh_url(url: &str) -> Result<SshUrl, String> {
    if let Some(rest) = url.strip_prefix("ssh://") {
        // ssh://[user@]host[:port]/path
        let (userhost, path) = rest
            .split_once('/')
            .ok_or_else(|| format!("Invalid SSH URL (missing path): {}", url))?;

        let (user, hostport) = if let Some((u, hp)) = userhost.split_once('@') {
            (Some(u.to_string()), hp)
        } else {
            (None, userhost)
        };

        let (host, port) = if let Some((h, p)) = hostport.split_once(':') {
            let port_num = p
                .parse::<u16>()
                .map_err(|_| format!("Invalid port in SSH URL: {}", p))?;
            (h.to_string(), Some(port_num))
        } else {
            (hostport.to_string(), None)
        };

        if host.is_empty() {
            return Err(format!("Empty host in SSH URL: {}", url));
        }

        Ok(SshUrl {
            user,
            host,
            port,
            path: format!("/{}", path),
        })
    } else if url.contains('@') && url.contains(':') && !url.contains("://") {
        // user@host:path (SCP-style)
        let (user_host, path) = url
            .split_once(':')
            .ok_or_else(|| format!("Invalid SCP-style SSH URL: {}", url))?;
        let (user, host) = user_host
            .split_once('@')
            .ok_or_else(|| format!("Invalid SCP-style SSH URL: {}", url))?;

        if host.is_empty() || path.is_empty() {
            return Err(format!("Invalid SCP-style SSH URL: {}", url));
        }

        Ok(SshUrl {
            user: Some(user.to_string()),
            host: host.to_string(),
            port: None,
            path: path.to_string(),
        })
    } else {
        Err(format!(
            "Not an SSH URL: {}. Use ssh://[user@]host[:port]/path or user@host:path",
            url
        ))
    }
}

/// An SSH pipe connection to a remote `lit serve --stdio` instance
pub struct SshPipe {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: BufWriter<std::process::ChildStdin>,
}

impl SshPipe {
    /// Open an SSH pipe to a remote repository
    pub fn open(parsed: &SshUrl) -> Result<Self, String> {
        let mut cmd = Command::new("ssh");

        // Disable interactive prompts for batch mode
        cmd.arg("-o").arg("BatchMode=yes");

        if let Some(port) = parsed.port {
            cmd.arg("-p").arg(port.to_string());
        }

        let target = if let Some(ref user) = parsed.user {
            format!("{}@{}", user, parsed.host)
        } else {
            parsed.host.clone()
        };
        cmd.arg(&target);

        // Remote command: cd to repo path and run lit serve --stdio
        let remote_cmd = format!("cd {} && lit serve --stdio", shell_escape(&parsed.path));
        cmd.arg(remote_cmd);

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn ssh: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture ssh stdout")?;
        let stdin = child.stdin.take().ok_or("Failed to capture ssh stdin")?;

        Ok(SshPipe {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
        })
    }

    /// Open a pipe directly to a `lit serve --stdio` process (for testing without SSH)
    pub fn open_local(repo_path: &std::path::Path) -> Result<Self, String> {
        // Find the lit binary: look in the same directory as the current executable,
        // which covers both `cargo run` and `cargo test` scenarios.
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Cannot find current executable: {}", e))?;
        let exe_dir = current_exe
            .parent()
            .ok_or("Cannot determine executable directory")?;

        // In test builds, the test binary is in target/debug/deps/ but
        // the lit binary is in target/debug/
        let lit_exe = if exe_dir.ends_with("deps") {
            exe_dir
                .parent()
                .unwrap()
                .join("lit")
                .with_extension(std::env::consts::EXE_EXTENSION)
        } else {
            exe_dir
                .join("lit")
                .with_extension(std::env::consts::EXE_EXTENSION)
        };

        if !lit_exe.exists() {
            return Err(format!(
                "lit binary not found at {}. Run `cargo build` first.",
                lit_exe.display()
            ));
        }

        let mut child = Command::new(lit_exe)
            .arg("serve")
            .arg("--stdio")
            .current_dir(repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn lit serve --stdio: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;

        Ok(SshPipe {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
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
            .map_err(|e| format!("Failed to write to SSH pipe: {}", e))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush SSH pipe: {}", e))?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read from SSH pipe: {}", e))?;

        if line.is_empty() {
            return Err("SSH pipe closed unexpectedly".to_string());
        }

        let resp: serde_json::Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("Invalid JSON from SSH pipe: {}", e))?;

        let status = resp.get("status").and_then(|v| v.as_u64()).unwrap_or(500) as u16;

        // The body field is a JSON string that needs to be parsed
        let body_str = resp.get("body").and_then(|v| v.as_str()).unwrap_or("{}");

        let body_json: serde_json::Value =
            serde_json::from_str(body_str).unwrap_or_else(|_| serde_json::json!({"raw": body_str}));

        Ok((status, body_json))
    }

    /// Close the SSH pipe gracefully.
    /// The `Drop` implementation will kill the process if not already exited.
    pub fn close(&mut self) {
        // Signal EOF to the remote process by closing our stdin handle,
        // using a zero-byte write attempt followed by checking the child status.
        let _ = self.child.try_wait();
    }
}

impl Drop for SshPipe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
        Err(format!("SSH transport error ({}): {}", status, msg))
    } else {
        Ok(())
    }
}

/// List refs from a remote server via SSH pipe
pub fn list_refs_ssh(pipe: &mut SshPipe, kind: &str) -> Result<Vec<RemoteRef>, String> {
    let path = format!("/api/v1/transport/refs?kind={}", kind);
    let (status, body) = pipe.request("GET", &path, "")?;
    check_status(status, &body)?;

    let refs = body
        .get("refs")
        .and_then(|v| v.as_array())
        .ok_or("Invalid refs response from SSH")?;

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

/// Read a branch ref from a remote server via SSH pipe
pub fn read_ref_ssh(pipe: &mut SshPipe, branch: &str) -> Result<String, String> {
    let path = format!("/api/v1/transport/refs/heads/{}", branch);
    let (status, body) = pipe.request("GET", &path, "")?;
    check_status(status, &body)?;
    body.get("hash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing hash in SSH response".to_string())
}

/// Read HEAD from a remote server via SSH pipe
pub fn read_head_ssh(pipe: &mut SshPipe) -> Result<String, String> {
    let (status, body) = pipe.request("GET", "/api/v1/transport/head", "")?;
    check_status(status, &body)?;
    body.get("head")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing head in SSH response".to_string())
}

/// Update a branch ref on a remote server via SSH pipe
pub fn update_ref_ssh(
    pipe: &mut SshPipe,
    branch: &str,
    hash: &str,
    force: bool,
) -> Result<(), String> {
    let path = format!("/api/v1/transport/refs/heads/{}", branch);
    let body = serde_json::json!({"hash": hash, "force": force}).to_string();
    let (status, resp) = pipe.request("PUT", &path, &body)?;
    check_status(status, &resp)
}

/// Negotiate which objects are needed via SSH pipe
pub fn negotiate_ssh(
    pipe: &mut SshPipe,
    wants: &[String],
    haves: &[String],
) -> Result<Vec<ObjectHash>, String> {
    let body = serde_json::json!({"wants": wants, "haves": haves}).to_string();
    let (status, resp) = pipe.request("POST", "/api/v1/transport/negotiate", &body)?;
    check_status(status, &resp)?;

    let needed = resp
        .get("needed")
        .and_then(|v| v.as_array())
        .ok_or("Invalid negotiate response from SSH")?;

    Ok(needed
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| ObjectHash::from_hex(s.to_string()))
        .collect())
}

/// Download objects from a remote server via SSH pipe
pub fn download_objects_ssh(
    pipe: &mut SshPipe,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
) -> Result<usize, String> {
    let mut count = 0;
    for hash in hashes {
        if local_store.exists(hash) {
            continue;
        }
        let path = format!("/api/v1/transport/objects/{}", hash.as_str());
        let (status, body) = pipe.request("GET", &path, "")?;
        check_status(status, &body)?;

        let b64_data = body
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or("Missing object data in SSH response")?;

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

/// Upload objects from a local store to a remote server via SSH pipe
pub fn upload_objects_ssh(
    pipe: &mut SshPipe,
    local_store: &ObjectStore,
    hashes: &[ObjectHash],
) -> Result<usize, String> {
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

        let body = serde_json::json!({"objects": objects_json}).to_string();
        let (status, resp) = pipe.request("POST", "/api/v1/transport/objects", &body)?;
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

/// Escape a path for use in a shell command
fn shell_escape(s: &str) -> String {
    // Use single quotes, escaping any single quotes in the string
    format!("'{}'", s.replace('\'', "'\\''"))
}
