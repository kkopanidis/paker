# Paker MCP server

Paker ships a read-only [Model Context Protocol](https://modelcontextprotocol.io) server as the `paker-mcp` binary. It exposes connection metadata and **local bucket index** tools so agents (Cursor, Claude Desktop, etc.) can inspect indexed S3 data without mutating buckets.

Live S3 listing (`list_buckets`, `list_objects`) is intentionally **not** exposed: credentials stay inside the desktop app vault and the MCP process only reads the on-disk index SQLite database.

## Build

```bash
cd src-tauri
cargo build --release --features mcp --bin paker-mcp
```

The release binary is bundled with the Tauri app on macOS at:

`Paker.app/Contents/MacOS/paker-mcp`

For development:

```bash
cargo run --features mcp --bin paker-mcp
```

## Configure Cursor / Claude Desktop

Copy one of the examples from [`mcp-example.json`](./mcp-example.json) into your MCP config.

Minimal production entry (macOS):

```json
{
  "mcpServers": {
    "paker": {
      "command": "/Applications/Paker.app/Contents/MacOS/paker-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `PAKER_DATA_DIR` | Override the Paker data directory (connections + index DB). Required if auto-detection fails. |
| `PAKER_MCP_ALLOWED_CONNECTIONS` | Comma-separated connection IDs to expose. Omit to allow all saved connections. |
| `RUST_LOG` | Tracing filter, e.g. `paker_mcp=debug,paker_lib=warn` |

Default data directory:

- **macOS:** `~/Library/Application Support/com.paker.app`
- **Portable mode:** `{exe_dir}/data` when `portable.txt` sits next to the binary

## Tools

| Tool | Purpose |
|------|---------|
| `list_connections` | Saved profiles (id, name, endpoint, region) — **no credentials** |
| `get_bucket_index_status` | Whether a bucket index exists and metadata (object count, timestamps) |
| `search_index` | Substring keyword search over indexed keys |
| `query_index` | Structured filters (prefix, LIKE pattern, size/date/storage class) |
| `get_bucket_report` | Aggregate stats (top prefixes, glacier counts, small files) |
| `explain_s3_error` | Map Paker/S3 error codes to user-facing guidance |
| `generate_cli_commands` | AWS CLI / rclone command suggestions for keys or prefixes |

All index tools require a **completed bucket index** built inside Paker first.

## Smart Pack models (desktop app)

On-device NL parsing is separate from MCP. To enable the LLM fallback in the desktop app:

1. Build with `--features llm` (optional; off in default CI builds).
2. Drop a quantised GGUF parser model into `{PAKER_DATA_DIR}/models/paker-parser.gguf`.
   Recommended weights: **Gemma 3 270M** or **Llama 3.2 1B** (Q4).
3. Restart Paker. Use **Smart Pack → Open models folder** to reveal the directory.

Embedding search (`paker-embed.gguf`, EmbeddingGemma 300M Q8) is reserved for a future phase.

## Security notes

- MCP is **read-only** — no delete, upload, or proposal execution.
- Connection allowlisting via `PAKER_MCP_ALLOWED_CONNECTIONS` is recommended on shared machines.
- The server never prints access keys; it reads only `connections.json` and `index/index.db`.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Index database is unavailable` | Open Paker, index the bucket, confirm `{data_dir}/index/index.db` exists |
| Empty connection list | Set `PAKER_DATA_DIR` to the same directory the desktop app uses |
| `Connection not found or not permitted` | Check connection id spelling or allowlist env var |

See also: [`docs/plans/phase-3-mcp-server.md`](./plans/phase-3-mcp-server.md)
