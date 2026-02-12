# PicoClaw Rust

Ultra-lightweight personal AI assistant framework, ported from Go to Rust.

## Overview

PicoClaw Rust is a complete rewrite of the PicoClaw AI assistant in Rust, maintaining the same ultra-efficient design philosophy while leveraging Rust's safety guarantees and zero-cost abstractions.

### Features

- **Ultra-Lightweight**: Minimal memory footprint with aggressive release optimizations
- **Async Runtime**: Built on Tokio for efficient concurrent operations
- **Multi-Provider Support**: Works with OpenRouter, Anthropic, OpenAI, Gemini, Zhipu, DeepSeek, and Groq
- **Tool System**: Extensible tool framework with bash, file operations, and web search
- **Gateway Support**: Telegram bot integration for remote access
- **Memory Persistence**: Session and long-term memory storage

## Building

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- A supported LLM API key (OpenRouter, Anthropic, etc.)

### Development Build

```bash
make build
# or
cargo build
```

### Release Build

```bash
make build-release
# or
cargo build --release
```

The release build uses aggressive optimizations:
- `opt-level = "z"` - Optimize for size
- `lto = true` - Link-time optimization
- `codegen-units = 1` - Single codegen unit for better optimization
- `strip = true` - Strip symbols
- `panic = "abort"` - Abort on panic (smaller binary)

### Installation

```bash
make install
```

This installs the binary to `~/.local/bin/picoclaw`.

## Usage

### Quick Start

1. **Initialize configuration**:
   ```bash
   picoclaw onboard
   ```

2. **Configure API keys** in `~/.picoclaw/config.json`:
   ```json
   {
     "agents": {
       "defaults": {
         "model": "anthropic/claude-sonnet-4",
         "max_tokens": 8192
       }
     },
     "providers": {
       "openrouter": {
         "api_key": "sk-or-v1-xxx"
       }
     }
   }
   ```

3. **Chat with the agent**:
   ```bash
   picoclaw agent -m "What is 2+2?"
   ```

### CLI Commands

| Command | Description |
|---------|-------------|
| `picoclaw onboard` | Initialize config and workspace |
| `picoclaw agent -m "..."` | Single message mode |
| `picoclaw agent` | Interactive chat mode |
| `picoclaw gateway` | Start Telegram bot gateway |
| `picoclaw status` | Show configuration status |

### Interactive Mode

```bash
picoclaw agent
```

In interactive mode:
- Type messages and press Enter to send
- Type `exit` or `quit` to end the session
- Press Ctrl+C to interrupt

## Configuration

Config file location: `~/.picoclaw/config.json`

### Full Configuration Example

```json
{
  "agents": {
    "defaults": {
      "workspace": "~/.picoclaw/workspace",
      "model": "anthropic/claude-sonnet-4",
      "max_tokens": 8192,
      "temperature": 0.7,
      "max_tool_iterations": 20
    }
  },
  "providers": {
    "openrouter": {
      "api_key": "sk-or-v1-xxx",
      "api_base": "https://openrouter.ai/api/v1"
    },
    "anthropic": {
      "api_key": "sk-ant-xxx"
    },
    "openai": {
      "api_key": "sk-xxx"
    },
    "gemini": {
      "api_key": "xxx"
    },
    "zhipu": {
      "api_key": "xxx",
      "api_base": "https://open.bigmodel.cn/api/paas/v4"
    },
    "deepseek": {
      "api_key": "xxx"
    },
    "groq": {
      "api_key": "gsk_xxx"
    }
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
      "search": {
        "api_key": "BSA...",
        "max_results": 5
      }
    }
  }
}
```

### Supported LLM Providers

| Provider | API Base | Notes |
|----------|----------|-------|
| OpenRouter | `openrouter.ai/api/v1` | Access to multiple models |
| Anthropic | `api.anthropic.com` | Claude models |
| OpenAI | `api.openai.com/v1` | GPT models |
| Gemini | `generativelanguage.googleapis.com` | Google's Gemini |
| Zhipu | `open.bigmodel.cn/api/paas/v4` | GLM models |
| DeepSeek | `api.deepseek.com` | DeepSeek models |
| Groq | `api.groq.com/openai/v1` | Fast inference |

### Workspace Structure

```
~/.picoclaw/workspace/
├── sessions/          # Conversation history
├── memory/           # Long-term memory (MEMORY.md)
├── AGENTS.md         # Agent behavior guide
├── IDENTITY.md       # Agent identity
├── SOUL.md           # Agent personality
├── TOOLS.md          # Tool descriptions
└── USER.md           # User preferences
```

## Development

### Running Tests

```bash
make test
# or with output
make test-verbose
```

### Linting

```bash
make lint
```

### Formatting

```bash
make fmt
```

### Full Quality Check

```bash
make check
```

### Project Structure

```
rust/
├── Cargo.toml           # Dependencies and build config
├── Makefile             # Build automation
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Library root
│   ├── config/          # Configuration loading
│   ├── llm/             # LLM provider implementations
│   ├── agent/           # Agent core and execution
│   ├── tools/           # Tool implementations
│   ├── gateway/         # Telegram integration
│   └── utils/           # Utility functions
└── tests/               # Integration tests
```

## Tools

PicoClaw includes several built-in tools:

| Tool | Description |
|------|-------------|
| `bash` | Execute shell commands |
| `read_file` | Read file contents |
| `write_file` | Write content to files |
| `list_files` | List directory contents |
| `search_files` | Search for patterns in files |
| `web_search` | Search the web (requires Brave API key) |
| `memory` | Store/retrieve long-term memory |

## Comparison with Go Version

| Aspect | Go | Rust |
|--------|-----|------|
| Memory safety | Runtime | Compile-time |
| Async model | Goroutines | Tokio async/await |
| Error handling | Multiple returns | Result type |
| Binary size | ~8MB | ~5MB (optimized) |
| Startup time | <1s | <1s |

## License

MIT License - See [LICENSE](../LICENSE) for details.

## Contributing

Contributions are welcome! Please ensure:
1. Code passes `make check`
2. New features include tests
3. Documentation is updated

## Acknowledgments

- Original [PicoClaw Go](https://github.com/sipeed/picoclaw) implementation
- Inspired by [nanobot](https://github.com/HKUDS/nanobot)
