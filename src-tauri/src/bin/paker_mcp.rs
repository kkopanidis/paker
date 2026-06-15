//! paker-mcp — Paker MCP server over stdio (JSON-RPC 2.0, read-only)
//!
//! Build:  `cargo build --features mcp -p paker`
//! Run:    `paker-mcp`                           (auto-resolves data dir)
//!         `PAKER_DATA_DIR=/path paker-mcp`      (explicit dir)
//!
//! Speaks the [Model Context Protocol](https://modelcontextprotocol.io) over
//! stdin/stdout using newline-delimited JSON-RPC 2.0.

use paker_lib::mcp_exports::{
    explain_error_code, generate_cli_commands, index_db_path_in, is_portable_mode,
    list_connections_from, CliGenerateInput, ConnectionProfile, IndexQuery, ObjectCacheManager,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

// ─── Data-directory resolution ───────────────────────────────────────────────

fn resolve_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(dir) = std::env::var("PAKER_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if is_portable_mode() {
        let exe = std::env::current_exe()?;
        let exe_dir = exe
            .parent()
            .ok_or_else(|| anyhow::anyhow!("executable has no parent directory"))?;
        return Ok(exe_dir.join("data"));
    }
    let base = dirs::data_dir().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot resolve platform data directory; \
             set PAKER_DATA_DIR to your Paker data directory"
        )
    })?;
    Ok(base.join("com.paker.app"))
}

fn allowed_ids() -> Option<Vec<String>> {
    std::env::var("PAKER_MCP_ALLOWED_CONNECTIONS")
        .ok()
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
}

fn filter_profiles(
    profiles: Vec<ConnectionProfile>,
    allowed: &Option<Vec<String>>,
) -> Vec<ConnectionProfile> {
    match allowed {
        None => profiles,
        Some(ids) => profiles.into_iter().filter(|p| ids.contains(&p.id)).collect(),
    }
}

// ─── Safe connection view (strips access_key_id) ────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeConnection<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<&'a str>,
    region: &'a str,
    force_path_style: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_bucket: Option<&'a str>,
}

impl<'a> From<&'a ConnectionProfile> for SafeConnection<'a> {
    fn from(p: &'a ConnectionProfile) -> Self {
        Self {
            id: &p.id,
            name: &p.name,
            endpoint: p.endpoint.as_deref(),
            region: &p.region,
            force_path_style: p.force_path_style,
            default_bucket: p.default_bucket.as_deref(),
        }
    }
}

// ─── Tool result helpers ─────────────────────────────────────────────────────

fn tool_ok(value: impl Serialize) -> Value {
    let text = serde_json::to_string(&value)
        .unwrap_or_else(|e| format!("{{\"serializationError\":\"{e}\"}}"));
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

fn tool_err(msg: impl std::fmt::Display) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("Error: {msg}") }], "isError": true })
}

// ─── Argument extraction helpers ─────────────────────────────────────────────

/// Return the first string value found for any of the given keys (camelCase then snake_case).
fn str_field(args: &Value, camel: &str, snake: &str) -> Option<String> {
    args.get(camel)
        .or_else(|| args.get(snake))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

fn require_str(args: &Value, camel: &str, snake: &str) -> Result<String, Value> {
    str_field(args, camel, snake)
        .ok_or_else(|| tool_err(format!("missing required field: {camel}")))
}

fn opt_u64(args: &Value, camel: &str, snake: &str) -> Option<u64> {
    args.get(camel)
        .or_else(|| args.get(snake))
        .and_then(Value::as_u64)
}

fn opt_u32_clamped(args: &Value, camel: &str, snake: &str, default: u32, max: u32) -> u32 {
    args.get(camel)
        .or_else(|| args.get(snake))
        .and_then(Value::as_u64)
        .map(|v| (v as u32).min(max))
        .unwrap_or(default)
}

fn resolve_profile<'a>(
    profiles: &'a [ConnectionProfile],
    id: &str,
) -> Result<&'a ConnectionProfile, Value> {
    profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| tool_err("Connection not found or not permitted"))
}

fn require_cache(cache: Option<&ObjectCacheManager>) -> Result<&ObjectCacheManager, Value> {
    cache.ok_or_else(|| tool_err("Index database is unavailable on this host"))
}

// ─── Individual tools ────────────────────────────────────────────────────────

fn tool_list_connections(profiles: &[ConnectionProfile]) -> Value {
    let safe: Vec<SafeConnection> = profiles.iter().map(SafeConnection::from).collect();
    tool_ok(safe)
}

fn tool_get_bucket_index_status(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    args: &Value,
) -> Value {
    let connection_id = match require_str(args, "connectionId", "connection_id") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let bucket = match require_str(args, "bucket", "bucket") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if resolve_profile(profiles, &connection_id).is_err() {
        return tool_err("Connection not found or not permitted");
    }
    let cache = match require_cache(cache) {
        Ok(c) => c,
        Err(e) => return e,
    };
    match cache.get_bucket_index_meta(&connection_id, &bucket) {
        Some(meta) => tool_ok(meta),
        None => tool_ok(Value::Null),
    }
}

fn tool_search_index(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    args: &Value,
) -> Value {
    let connection_id = match require_str(args, "connectionId", "connection_id") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let bucket = match require_str(args, "bucket", "bucket") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let query = match require_str(args, "query", "query") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if resolve_profile(profiles, &connection_id).is_err() {
        return tool_err("Connection not found or not permitted");
    }
    let cache = match require_cache(cache) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let limit = opt_u32_clamped(args, "limit", "limit", 50, 200);
    let offset = opt_u32_clamped(args, "offset", "offset", 0, u32::MAX);

    match cache.search_bucket_index(&connection_id, &bucket, &query, limit, offset) {
        Ok(results) => tool_ok(results),
        Err(e) => tool_err(e),
    }
}

fn tool_query_index(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    args: &Value,
) -> Value {
    let connection_id = match require_str(args, "connectionId", "connection_id") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let bucket = match require_str(args, "bucket", "bucket") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if resolve_profile(profiles, &connection_id).is_err() {
        return tool_err("Connection not found or not permitted");
    }
    let cache = match require_cache(cache) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let storage_class = args
        .get("storageClass")
        .or_else(|| args.get("storage_class"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        });

    let limit = opt_u32_clamped(args, "limit", "limit", 100, 200);
    let offset = opt_u32_clamped(args, "offset", "offset", 0, u32::MAX);

    let index_query = IndexQuery {
        prefix: str_field(args, "prefix", "prefix"),
        key_pattern: str_field(args, "keyPattern", "key_pattern"),
        min_size: opt_u64(args, "minSize", "min_size"),
        max_size: opt_u64(args, "maxSize", "max_size"),
        modified_after: str_field(args, "modifiedAfter", "modified_after"),
        modified_before: str_field(args, "modifiedBefore", "modified_before"),
        storage_class,
        limit,
        offset,
    };

    match cache.query_bucket_index(&connection_id, &bucket, &index_query) {
        Ok(results) => tool_ok(results),
        Err(e) => tool_err(e),
    }
}

fn tool_get_bucket_report(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    args: &Value,
) -> Value {
    let connection_id = match require_str(args, "connectionId", "connection_id") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let bucket = match require_str(args, "bucket", "bucket") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if resolve_profile(profiles, &connection_id).is_err() {
        return tool_err("Connection not found or not permitted");
    }
    let cache = match require_cache(cache) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let top_n = opt_u32_clamped(args, "topN", "top_n", 10, 50);

    match cache.build_bucket_report(&connection_id, &bucket, top_n) {
        Ok(report) => tool_ok(report),
        Err(e) => tool_err(e),
    }
}

fn tool_explain_s3_error(args: &Value) -> Value {
    let code = match require_str(args, "code", "code") {
        Ok(v) => v,
        Err(e) => return e,
    };
    tool_ok(explain_error_code(&code))
}

fn tool_generate_cli_commands(profiles: &[ConnectionProfile], args: &Value) -> Value {
    let connection_id = match require_str(args, "connectionId", "connection_id") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let bucket = match require_str(args, "bucket", "bucket") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let profile = match resolve_profile(profiles, &connection_id) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let keys: Vec<String> = args
        .get("keys")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let input = CliGenerateInput {
        tool: str_field(args, "tool", "tool"),
        connection_id: connection_id.clone(),
        connection_name: Some(profile.name.clone()),
        endpoint: profile.endpoint.clone(),
        bucket,
        prefix: str_field(args, "prefix", "prefix"),
        keys,
    };

    tool_ok(generate_cli_commands(&input))
}

// ─── Tool dispatcher ─────────────────────────────────────────────────────────

fn call_tool(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    name: &str,
    args: Value,
) -> Value {
    match name {
        "list_connections" => tool_list_connections(profiles),
        "get_bucket_index_status" => tool_get_bucket_index_status(profiles, cache, &args),
        "search_index" => tool_search_index(profiles, cache, &args),
        "query_index" => tool_query_index(profiles, cache, &args),
        "get_bucket_report" => tool_get_bucket_report(profiles, cache, &args),
        "explain_s3_error" => tool_explain_s3_error(&args),
        "generate_cli_commands" => tool_generate_cli_commands(profiles, &args),
        _ => tool_err(format!("Unknown tool: {name}")),
    }
}

// ─── Tool schema definitions ─────────────────────────────────────────────────

fn tools_list() -> Value {
    json!([
        {
            "name": "list_connections",
            "description": "List all saved S3 connection profiles (id, name, endpoint, region, defaultBucket). Credentials are never included.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "get_bucket_index_status",
            "description": "Check whether a bucket has a completed local index and return metadata (status, objectCount, timestamps).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "connectionId": { "type": "string", "description": "Connection profile ID" },
                    "bucket":       { "type": "string", "description": "Bucket name" }
                },
                "required": ["connectionId", "bucket"]
            }
        },
        {
            "name": "search_index",
            "description": "Substring keyword search across locally-indexed object keys. Requires a completed bucket index.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "connectionId": { "type": "string" },
                    "bucket":       { "type": "string" },
                    "query":        { "type": "string", "description": "Substring to search for in object keys" },
                    "limit":        { "type": "integer", "default": 50,  "maximum": 200 },
                    "offset":       { "type": "integer", "default": 0 }
                },
                "required": ["connectionId", "bucket", "query"]
            }
        },
        {
            "name": "query_index",
            "description": "Structured filter query across the local bucket index. Supports prefix, SQL LIKE key patterns, size range, date range, and storage class filters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "connectionId":   { "type": "string" },
                    "bucket":         { "type": "string" },
                    "prefix":         { "type": "string",  "description": "Key prefix (e.g. 'logs/')" },
                    "keyPattern":     { "type": "string",  "description": "SQL LIKE pattern (e.g. '%.log.gz')" },
                    "minSize":        { "type": "integer", "description": "Minimum size in bytes" },
                    "maxSize":        { "type": "integer", "description": "Maximum size in bytes" },
                    "modifiedAfter":  { "type": "string",  "description": "ISO date, e.g. '2024-01-01'" },
                    "modifiedBefore": { "type": "string" },
                    "storageClass":   {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "e.g. ['STANDARD', 'STANDARD_IA', 'GLACIER']"
                    },
                    "limit":  { "type": "integer", "default": 100, "maximum": 200 },
                    "offset": { "type": "integer", "default": 0 }
                },
                "required": ["connectionId", "bucket"]
            }
        },
        {
            "name": "get_bucket_report",
            "description": "Aggregate storage statistics for an indexed bucket: total objects/bytes, top prefixes by size, Glacier counts, small-file counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "connectionId": { "type": "string" },
                    "bucket":       { "type": "string" },
                    "topN":         { "type": "integer", "default": 10, "maximum": 50, "description": "Number of top prefixes to return" }
                },
                "required": ["connectionId", "bucket"]
            }
        },
        {
            "name": "explain_s3_error",
            "description": "Return a human-readable explanation and remediation suggestions for a Paker/S3 error code.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "e.g. 'accessDenied', 'bucketNotFound', 'network', 'invalidEndpoint'" }
                },
                "required": ["code"]
            }
        },
        {
            "name": "generate_cli_commands",
            "description": "Generate aws-cli and rclone command suggestions for listing, syncing, or downloading S3 objects. Credentials are NOT embedded — commands reference named AWS profiles or rclone remotes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "connectionId": { "type": "string" },
                    "bucket":       { "type": "string" },
                    "prefix":       { "type": "string",  "description": "Key prefix (optional)" },
                    "keys":         {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Specific keys (optional). Empty = generate prefix-level commands."
                    },
                    "tool": {
                        "type": "string",
                        "enum": ["aws", "rclone"],
                        "description": "Restrict to a single CLI tool (optional, default = both)"
                    }
                },
                "required": ["connectionId", "bucket"]
            }
        }
    ])
}

// ─── JSON-RPC 2.0 dispatch ───────────────────────────────────────────────────

fn dispatch(
    profiles: &[ConnectionProfile],
    cache: Option<&ObjectCacheManager>,
    method: &str,
    params: Value,
    id: Value,
) -> Value {
    match method {
        "initialize" => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "paker",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })
        }
        "tools/list" => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools_list() }
            })
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = call_tool(profiles, cache, name, args);
            json!({ "jsonrpc": "2.0", "id": id, "result": result })
        }
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    }
}

// ─── Server I/O loop ─────────────────────────────────────────────────────────

fn write_line(out: &mut impl Write, value: &Value) -> std::io::Result<()> {
    let bytes =
        serde_json::to_vec(value).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    out.flush()
}

fn run_server(profiles: Vec<ConnectionProfile>, cache: Option<ObjectCacheManager>) {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, "stdin read error");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "JSON parse error");
                let _ = write_line(
                    &mut out,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": "Parse error" }
                    }),
                );
                continue;
            }
        };

        // Notifications have no id — ignore them silently.
        let id = match req.get("id") {
            None | Some(Value::Null) => continue,
            Some(v) => v.clone(),
        };

        let method = req
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = req.get("params").cloned().unwrap_or(json!({}));

        tracing::debug!(method = %method, "request");

        let response = dispatch(&profiles, cache.as_ref(), &method, params, id);

        if let Err(e) = write_line(&mut out, &response) {
            tracing::error!(error = %e, "stdout write error");
            break;
        }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("paker_mcp=info,paker_lib=warn"));
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_target(false)
        .try_init();

    let data_dir = match resolve_data_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("paker-mcp: failed to resolve data directory: {e}");
            eprintln!("paker-mcp: set PAKER_DATA_DIR to your Paker data directory and retry");
            std::process::exit(1);
        }
    };

    tracing::info!(data_dir = %data_dir.display(), "paker-mcp starting");

    let allowed = allowed_ids();
    let profiles = match list_connections_from(&data_dir) {
        Ok(p) => filter_profiles(p, &allowed),
        Err(e) => {
            tracing::warn!(error = %e, "failed to load connections; serving empty list");
            Vec::new()
        }
    };

    tracing::info!(
        count = profiles.len(),
        restricted = allowed.is_some(),
        "loaded connection profiles"
    );

    let cache = match index_db_path_in(&data_dir) {
        Ok(db_path) => match ObjectCacheManager::open(db_path) {
            Ok(c) => {
                tracing::info!("index database opened");
                Some(c)
            }
            Err(e) => {
                tracing::warn!(error = %e, "index database unavailable; index tools disabled");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "cannot resolve index DB path; index tools disabled");
            None
        }
    };

    run_server(profiles, cache);
}
