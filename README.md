# ZeptoClaw

Ultra-lightweight personal AI assistant framework in Rust. The smallest, fastest, safest member of the Claw family.

## Features

- **Ultra-Lightweight**: ~4MB binary, ~6MB RSS, <50ms startup
- **Async Runtime**: Built on Tokio for efficient concurrent operations
- **Multi-Provider**: Anthropic Claude and OpenAI (with OpenAI-compatible endpoints)
- **Container Isolation**: Native, Docker, or Apple Container runtimes for shell sandboxing
- **Containerized Gateway**: Full agent isolation per request via Docker or Apple Containers
- **Tool System**: 13 tools including shell, filesystem, web, WhatsApp, Google Sheets, cron, spawn
- **Multi-Channel**: Telegram, Slack (outbound), CLI
- **Skills System**: Markdown-based skill files for extending agent capabilities
- **Heartbeat Service**: Periodic background task execution from HEARTBEAT.md
- **Security Hardened**: SSRF prevention, path traversal detection, shell command blocklist, mount validation

## Quick Start

```bash
# Build
cargo build --release

# Interactive setup
./target/release/zeptoclaw onboard

# Chat with agent
./target/release/zeptoclaw agent -m "Hello"

# Start Telegram bot gateway
./target/release/zeptoclaw gateway

# Gateway with container isolation
./target/release/zeptoclaw gateway --containerized
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `zeptoclaw onboard` | Interactive configuration setup |
| `zeptoclaw agent -m "..."` | Single message mode |
| `zeptoclaw agent` | Interactive chat mode |
| `zeptoclaw gateway` | Start channel gateway (Telegram bot) |
| `zeptoclaw gateway --containerized` | Gateway with container isolation (auto-detect) |
| `zeptoclaw gateway --containerized docker` | Force Docker backend |
| `zeptoclaw gateway --containerized apple` | Force Apple Container (macOS 15+) |
| `zeptoclaw heartbeat` | Trigger heartbeat check manually |
| `zeptoclaw heartbeat --show` | Show heartbeat file contents |
| `zeptoclaw heartbeat --edit` | Edit heartbeat file in $EDITOR |
| `zeptoclaw skills list` | List available skills |
| `zeptoclaw skills show <name>` | Show skill content |
| `zeptoclaw skills create <name>` | Create a new skill |
| `zeptoclaw status` | Show configuration status |

## Configuration

Config file: `~/.zeptoclaw/config.json`

```json
{
  "agents": {
    "defaults": {
      "workspace": "~/.zeptoclaw/workspace",
      "model": "anthropic/claude-sonnet-4",
      "max_tokens": 8192,
      "temperature": 0.7,
      "max_tool_iterations": 20
    }
  },
  "providers": {
    "anthropic": { "api_key": "sk-ant-xxx" },
    "openai": { "api_key": "sk-xxx" }
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "123456:ABC...",
      "allow_from": ["123456789"]
    }
  },
  "tools": {
    "web": {
      "search": { "api_key": "BSA...", "max_results": 5 }
    },
    "whatsapp": {
      "phone_number_id": "123456789",
      "access_token": "EAA..."
    },
    "google_sheets": {
      "access_token": "ya29..."
    }
  },
  "runtime": {
    "runtime_type": "native",
    "docker": { "image": "alpine:latest", "memory_limit": "512m" }
  },
  "heartbeat": {
    "enabled": true,
    "interval_secs": 1800
  },
  "skills": {
    "enabled": true,
    "always_load": ["github"]
  }
}
```

Environment variables override config values:
- `ZEPTOCLAW_PROVIDERS_ANTHROPIC_API_KEY`
- `ZEPTOCLAW_PROVIDERS_OPENAI_API_KEY`
- `ZEPTOCLAW_CHANNELS_TELEGRAM_BOT_TOKEN`

## Tools

| Tool | Description | Requires |
|------|-------------|----------|
| `shell` | Execute shell commands (with runtime isolation) | - |
| `read_file` | Read file contents | - |
| `write_file` | Write content to files | - |
| `list_dir` | List directory contents | - |
| `edit_file` | Find-and-replace in files | - |
| `web_search` | Search the web via Brave API | Brave API key |
| `web_fetch` | Fetch and extract URL content | - |
| `whatsapp_send` | Send WhatsApp messages | Meta Cloud API credentials |
| `google_sheets` | Read/write Google Sheets | Google API credentials |
| `message` | Send messages to chat channels | - |
| `memory_get` / `memory_search` | Read and search workspace memory | - |
| `cron` | Schedule recurring tasks | - |
| `spawn` | Delegate background tasks | - |

## Architecture

```
src/
├── agent/          # Agent loop, context builder
├── bus/            # Async message bus (pub/sub)
├── channels/       # Input channels (Telegram, Slack, CLI)
├── config/         # Configuration types and loading
├── cron/           # Persistent cron scheduler
├── gateway/        # Containerized agent proxy
├── heartbeat/      # Periodic background task service
├── memory/         # Workspace memory (markdown-based)
├── providers/      # LLM providers (Claude, OpenAI)
├── runtime/        # Container runtimes (Native, Docker, Apple)
├── security/       # Shell blocklist, path validation, mount policy
├── session/        # Session and message persistence
├── skills/         # Markdown-based skill system
├── tools/          # 13 agent tools
├── utils/          # Utility functions
├── error.rs        # Error types
├── lib.rs          # Library exports
└── main.rs         # CLI entry point
```

## Security

ZeptoClaw uses defense-in-depth:

1. **Container Isolation** (Docker/Apple Container) - process, filesystem, and network isolation for shell commands
2. **Containerized Gateway** - full agent isolation per request with semaphore-based concurrency
3. **Shell Command Blocklist** - regex patterns blocking dangerous commands (rm -rf, reverse shells, etc.)
4. **Path Traversal Protection** - symlink escape detection, workspace-scoped filesystem tools
5. **SSRF Prevention** - DNS pre-resolution against private IPs, redirect host validation, streaming body limits
6. **Input Validation** - URL path injection prevention, spreadsheet ID validation, mount allowlist
7. **Rate Limiting** - cron job caps (50 active, 60s minimum interval), spawn recursion prevention

## Containerized Gateway

Run each agent request in an isolated container:

```bash
# Auto-detect best backend
zeptoclaw gateway --containerized

# Build the container image first
docker build -t zeptoclaw:latest .
```

The gateway uses stdin/stdout IPC with the containerized agent, supports concurrent request processing via semaphore, and validates all mounts against a security allowlist.

## Multi-Tenant Deployment

Run multiple tenants on a single VPS using container-per-tenant isolation. Each tenant gets their own container, config, and data volume. ZeptoClaw is ~6MB RSS, so hundreds of tenants fit on a small VPS.

```bash
./scripts/add-tenant.sh shop-ahmad "BOT_TOKEN" "API_KEY"
./scripts/generate-compose.sh > docker-compose.multi-tenant.yml
docker compose -f docker-compose.multi-tenant.yml up -d
```

See [docs/MULTI-TENANT.md](docs/MULTI-TENANT.md) for full guide.

## Development

```bash
# Run all tests (498 total: 442 lib + 56 integration)
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## Performance

Verified on Apple Silicon (release build):

| Metric | Value |
|--------|-------|
| Binary size | ~4MB |
| Startup time | ~50ms |
| Memory (RSS) | ~6MB |
| Test suite | 498 tests (442 lib + 56 integration) |

## License

MIT
