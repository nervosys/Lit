use crate::errors::LitError;
use crate::response::{CommandResponse, OutputFormat};
use serde::Serialize;

/// Output format including MsgPack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    JsonPretty,
    Human,
    MsgPack,
}

impl Format {
    /// Resolve from CLI flags, env var, and config.
    ///
    /// `pretty` selects indented JSON (token-heavy, for humans). The default
    /// JSON output is compact for agent token efficiency.
    pub fn resolve(json: bool, human: bool, output: Option<&str>, pretty: bool) -> Self {
        if human {
            return Format::Human;
        }
        if json {
            return if pretty { Format::JsonPretty } else { Format::Json };
        }
        if let Some(fmt) = output {
            return match fmt {
                "human" | "text" => Format::Human,
                "msgpack" => Format::MsgPack,
                "json-pretty" | "pretty" => Format::JsonPretty,
                _ if pretty => Format::JsonPretty,
                _ => Format::Json,
            };
        }
        match std::env::var("LIT_OUTPUT").as_deref() {
            Ok("human") | Ok("text") => Format::Human,
            Ok("msgpack") => Format::MsgPack,
            Ok("json-pretty") | Ok("pretty") => Format::JsonPretty,
            _ if pretty => Format::JsonPretty,
            _ => Format::Json,
        }
    }

    /// Convert to OutputFormat for backward compatibility
    pub fn to_output_format(self) -> OutputFormat {
        match self {
            Format::Json | Format::JsonPretty | Format::MsgPack => OutputFormat::Json,
            Format::Human => OutputFormat::Human,
        }
    }
}

/// Format a response in the specified format (including MsgPack)
pub fn format_response<R: CommandResponse + Serialize>(response: &R, format: Format) -> Vec<u8> {
    match format {
        Format::Json => response.to_json_output().into_bytes(),
        Format::JsonPretty => response.to_json_output_pretty().into_bytes(),
        Format::Human => response.human_readable().into_bytes(),
        Format::MsgPack => {
            let data = serde_json::to_value(response).unwrap_or(serde_json::Value::Null);
            let envelope = MsgPackEnvelope {
                status: "ok",
                command: response.command_name(),
                data,
            };
            rmp_serde::to_vec(&envelope).unwrap_or_default()
        }
    }
}

/// Format an error in the specified format (including MsgPack)
pub fn format_error(error: &LitError, command: &str, format: Format) -> Vec<u8> {
    let err_obj = serde_json::json!({
        "status": "error",
        "command": command,
        "error": {
            "code": error.error_code(),
            "message": error.user_message(),
            "suggestions": error.suggestions(),
        }
    });

    match format {
        Format::Json => serde_json::to_string(&err_obj)
            .unwrap_or_default()
            .into_bytes(),
        Format::JsonPretty => serde_json::to_string_pretty(&err_obj)
            .unwrap_or_default()
            .into_bytes(),
        Format::Human => {
            let mut out = format!("error: {}", error.user_message());
            let suggestions = error.suggestions();
            if !suggestions.is_empty() {
                out.push_str("\n\nhint:");
                for s in suggestions {
                    out.push_str(&format!("\n  {}", s));
                }
            }
            out.into_bytes()
        }
        Format::MsgPack => rmp_serde::to_vec(&err_obj).unwrap_or_default(),
    }
}

/// Wrapper for MsgPack serialization
#[derive(Serialize)]
struct MsgPackEnvelope<'a> {
    status: &'a str,
    command: &'a str,
    data: serde_json::Value,
}
