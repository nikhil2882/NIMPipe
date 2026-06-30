# NIMPipe

**A local proxy that makes NVIDIA NIM models work with any OpenAI-compatible tool.**

NVIDIA NIM offers powerful LLMs, but each model has its own quirks for enabling features like thinking/reasoning modes. NIMPipe sits between your tools and NVIDIA NIM, normalizing these differences so you can use models like Kimi K2.6, MiniMax M3, and GLM-5.1 with Warp, OpenWebUI, or any OpenAI-compatible client — no per-model workarounds needed.

## The Problem

Every NVIDIA NIM model handles reasoning controls differently:

| Model | Reasoning Parameter |
|---|---|
| `moonshotai/kimi-k2.6` | `chat_template_kwargs.thinking = true` |
| `minimaxai/minimax-m3` | `chat_template_kwargs.thinking_mode = "enabled"` |
| Some models | Return HTTP `202` requiring async polling |

Your tools don't know about these differences. NIMPipe does.

## How It Works

```
Your Tool (Warp, OpenWebUI, etc.)
  │
  ▼  Standard OpenAI API request
NIMPipe (localhost:8787)
  │
  ├─ Looks up model alias (e.g. "kimi-k2.6-thinking")
  ├─ Injects model-specific parameters
  ├─ Strips unsupported parameters
  ├─ Clamps max_tokens to model limits
  │
  ▼  Transformed request
NVIDIA NIM API
```

You point your tools at `http://localhost:8787/v1` instead of the NVIDIA API directly. NIMPipe handles all the translation.

## Features

- **OpenAI-compatible API** — drop-in replacement endpoint for `/v1/models` and `/v1/chat/completions`
- **Model aliases** — expose friendly names like `kimi-k2.6-thinking` that map to the correct backend model with reasoning enabled
- **Parameter injection** — automatically inject model-specific params (supports nested keys like `chat_template_kwargs.thinking`)
- **Parameter stripping** — remove unsupported params per model (e.g. MiniMax doesn't support `stop`)
- **SSE stream transformation** — maps `reasoning_content` to `content` for clients that expect standard OpenAI format
- **Async 202 polling** — transparently handles models that return `202 Accepted` with exponential backoff
- **Mission control UI** — dark-themed dashboard to manage models, run test calls, and inspect events
- **Hot reload** — edit config and reload without restarting the server
- **Single binary** — web UI compiled into the binary, zero external dependencies at runtime

## Quick Start

### 1. Build

```bash
cargo build --release
```

### 2. Set your API key

```bash
export NIMPIPE_NVIDIA_API_KEY="nvapi-..."
```

### 3. Run

```bash
./target/release/nimpipe start --foreground
```

### 4. Open the dashboard

http://localhost:8787

### 5. Configure your tool

Point any OpenAI-compatible client to:

```
Base URL: http://localhost:8787/v1
API Key:  (leave empty or anything — NIMPipe uses the env var)
```

## Configuration

Config files live in OS-standard directories:

| OS | Path |
|---|---|
| macOS | `~/Library/Application Support/nimpipe/` |
| Linux | `~/.config/nimpipe/` |

Two files:

- `config.toml` — server host/port, timeouts, log level
- `models.toml` — model registry (editable via UI or by hand)

### Model Registry

Each model entry in `models.toml`:

```toml
[[models]]
openai_id = "kimi-k2.6-thinking"          # What your tools see
backend_id = "moonshotai/kimi-k2.6"        # Actual NVIDIA model ID
description = "Kimi K2.6 with reasoning"
max_tokens_cap = 65536
supports_streaming = true
supports_tools = true

[models.injected_params]
"chat_template_kwargs.thinking" = true     # Nested key injection
```

**Fields:**

| Field | Purpose |
|---|---|
| `openai_id` | Model name exposed to clients |
| `backend_id` | Upstream NVIDIA NIM model ID |
| `max_tokens_cap` | Clamps client `max_tokens` |
| `default_params` | Applied when client omits them |
| `injected_params` | Always merged into request (supports dotted keys) |
| `strip_params` | Removed before forwarding |
| `supports_streaming` | Reject streaming if `false` |
| `supports_tools` | Capability flag |
| `status_poll_path` | Path template for 202 polling |

### Server Config

```toml
[server]
host = "127.0.0.1"
port = 8787

[timeouts]
request_seconds = 120
streaming_seconds = 300

[logging]
level = "info"
debug_mode = false
```

## Shipped Models

NIMPipe ships with these defaults:

| Alias | Backend | Notes |
|---|---|---|
| `kimi-k2.6` | `moonshotai/kimi-k2.6` | Base model |
| `kimi-k2.6-thinking` | `moonshotai/kimi-k2.6` | Reasoning enabled |
| `minimax-m3` | `minimaxai/minimax-m3` | Strips unsupported params |
| `minimax-m3-thinking` | `minimaxai/minimax-m3` | Reasoning enabled |
| `glm-5.1` | `z-ai/glm-5.1` | Zhipu GLM |

## Architecture

```
src/
├── main.rs               # Entry point, wires everything together
├── cli.rs                # CLI args (clap)
├── config.rs             # TOML config loading/saving
├── logging.rs            # Dual logging: stdout + rotating JSON files
├── models.rs             # Model registry
├── proxy.rs              # HTTP client to NVIDIA NIM + 202 polling
├── server.rs             # Axum router + all handlers
├── transform.rs          # Request transformation pipeline
└── transform_response.rs # SSE stream transformation
```

**Tech stack:** Axum, Tokio, reqwest, clap, serde, tracing

## Security

- Listens on `127.0.0.1` by default — local use only
- API key is read from `NIMPIPE_NVIDIA_API_KEY` env var, never written to disk
- No authentication on the local proxy (don't expose to the network)
- Message content not logged unless debug mode is enabled

## Development

```bash
# Run in development
NIMPIPE_NVIDIA_API_KEY=nvapi-... cargo run -- start --foreground

# Run tests
cargo test
```

## License

MIT
