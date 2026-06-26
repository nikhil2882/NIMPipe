# NIMPipe

A local OpenAI-compatible proxy for NVIDIA NIM text LLMs. It normalizes per-model quirks (like `chat_template_kwargs` for thinking/reasoning) and gives you a mission-control UI to manage models, run test calls, and inspect recent events.

## Why

NVIDIA NIM exposes models through an OpenAI-compatible API, but details like reasoning controls vary by model:

- `moonshotai/kimi-k2.6` uses `chat_template_kwargs.thinking`
- `minimaxai/minimax-m3` uses `chat_template_kwargs.thinking_mode`
- Some models can return HTTP `202` with async status polling

NIMPipe hides these differences from your tools. You pick a model alias like `kimi-k2.6-thinking` in Warp or OpenWebUI, and NIMPipe injects the right upstream parameters.

## Features

- **OpenAI-compatible endpoints**: `/v1/models`, `/v1/chat/completions` (streaming + non-streaming)
- **Per-model registry** in TOML, editable through the web UI
- **Thinking variants** as separate model aliases
- **Async `202` polling** for non-streaming requests (configurable per model)
- **Test calls** from the UI to verify a model responds
- **Config reload** without restart
- **Single binary** with embedded web UI
- **macOS + Linux** config directories via the `dirs` crate

## Quick start

1. Build the binary:

```bash
cargo build --release
```

2. Set your NVIDIA API key:

```bash
export NIMPIPE_NVIDIA_API_KEY="nvapi-..."
```

3. Start the server:

```bash
./target/release/nimpipe start --foreground
```

4. Open the UI: http://localhost:8787

5. Point your OpenAI-compatible tool to:

```
http://localhost:8787/v1
```

## Configuration

Config files are stored in OS-appropriate directories:

- **macOS**: `~/Library/Application Support/nimpipe/`
- **Linux**: `~/.config/nimpipe/`

Files:

- `config.toml` — port, timeouts, log level
- `models.toml` — model registry

Edit `models.toml` in the UI or by hand, then click **Reload Config** or restart.

### Model registry example

```toml
[[models]]
openai_id = "kimi-k2.6-thinking"
backend_id = "moonshotai/kimi-k2.6"
description = "Moonshot Kimi K2.6 with reasoning enabled"
max_tokens_cap = 65536
supports_streaming = true
supports_tools = true

[models.injected_params]
"chat_template_kwargs.thinking" = true
```

Fields:

- `openai_id` — the model name your tools see
- `backend_id` — the upstream NVIDIA model ID
- `max_tokens_cap` — clamps the client's `max_tokens`
- `default_params` — applied when the client omits them
- `injected_params` — merged into the upstream body (supports dotted keys like `chat_template_kwargs.thinking`)
- `strip_params` — removed from the client request before forwarding
- `supports_streaming` / `supports_tools` — capability flags
- `status_poll_path` — path template for `202` polling, e.g. `/v1/status/{request_id}`

## Async `202` polling

Some NVIDIA models return HTTP `202` and a `requestId` that must be polled. NIMPipe handles this for non-streaming requests when `status_poll_path` is configured on the model. Streaming requests that receive `202` return an error because streaming and polling do not mix.

Default models ship without a `status_poll_path` because the exact polling URL is not consistently documented. If you need `202` handling, add the correct `status_poll_path` for that model.

## CLI commands

```bash
nimpipe start --foreground    # run in the terminal
nimpipe service install       # coming soon
nimpipe service uninstall     # coming soon
```

`stop`, `status`, `logs`, and `reload` are stubbed in v1.

## Development

Run tests:

```bash
cargo test
```

Run in development:

```bash
NIMPIPE_NVIDIA_API_KEY=nvapi-... cargo run -- start --foreground
```

## Security notes

- The local proxy does not require authentication. Run it only on `127.0.0.1` unless you know what you are doing.
- The NVIDIA API key is read from the `NIMPIPE_NVIDIA_API_KEY` environment variable only. It is never written to disk.
- Message content is not logged by default. Enable **Debug mode** in the UI only for troubleshooting.

## License

MIT
