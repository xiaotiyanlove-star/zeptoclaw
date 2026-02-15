# ZeptoClaw vs OpenClaw vs PicoClaw vs NanoClaw

> Factual comparison based on source code analysis. Last updated: 2026-02-15.

Four open-source projects solving the same problem — a self-hosted AI assistant you control — with different tradeoffs.

## At a Glance

| | **OpenClaw** | **PicoClaw** | **NanoClaw** | **ZeptoClaw** |
|---|---|---|---|---|
| **Language** | TypeScript / Node.js | Go | TypeScript / Node.js | Rust |
| **Binary / Install** | ~100MB+ (Node + 53 deps) | Single binary (<10MB) | Node.js + 10 deps | Single binary (~4MB) |
| **Memory (RSS)** | ~200MB+ | <10MB | ~50MB+ (Node + containers) | ~6MB |
| **Startup** | Seconds | <1s (on 0.6GHz SBC) | Seconds | ~50ms |
| **Built-in tools** | 52 skills | 12 tools | 0 (Claude Agent SDK) | 17 tools + MCP client |
| **Channels** | 14 + 32 extensions | 10 | 1 (WhatsApp) | 5 |
| **LLM providers** | Anthropic, OpenAI | 5-7 (Claude, OpenAI, Zhipu, Deepseek, Groq, OpenRouter) | Claude only | Claude, OpenAI + retry/fallback stack |
| **Container isolation** | Docker (deployment only) | None | Yes (per-agent, Apple/Docker) | Yes (per-command, Apple/Docker) |
| **Codebase** | ~465K lines | ~20.6K lines | ~3.4K lines | ~11K lines |
| **Tests** | 949 test files | 222 tests | ~413 assertions | 1,314 tests |
| **License** | MIT | MIT | MIT | Apache 2.0 |

## Tools & Capabilities

| Capability | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| Shell execution | Yes | Yes | Via Claude Agent SDK | Yes (with runtime isolation) |
| Filesystem (read/write/edit) | Yes | Yes (5 tools) | Via Claude Agent SDK | Yes (3 tools) |
| Web search + fetch | Yes | Yes | Via skills | Yes (SSRF-protected) |
| Memory / context persistence | Vector embeddings (SQLite) | Markdown-based | Per-group CLAUDE.md + SQLite | Long-term KV store + workspace memory |
| Cron / scheduling | Yes | Yes | Yes (cron-parser) | Yes |
| Background tasks (spawn) | Yes | Yes | No | Yes (spawn + delegate) |
| Agent swarms / delegation | Multi-agent routing | Subagent manager | Yes (Claude Agent Teams) | DelegateTool with tool whitelists |
| Hardware I/O (I2C, SPI) | No | Yes | No | No |
| Browser control | Yes (CDP) | No | Via skill (agent-browser) | No |
| Google Sheets | No | No | No | Yes |
| WhatsApp tool | Yes (Baileys) | Yes | Yes (Baileys, primary channel) | Yes (Cloud API) |
| MCP client | No | No | Yes (via Claude Agent SDK) | Yes (JSON-RPC 2.0) |
| Plugin system | Yes (32 extensions) | No | Skills (markdown-based) | Yes (JSON manifest) |
| Batch processing | No | No | No | Yes (text/JSONL) |
| Agent templates | No | No | No | Yes (4 built-in + custom) |
| Tool approval gate | No | No | No | Yes |
| Token budget tracking | No | No | No | Yes |
| Cost tracking | No | No | No | Yes (8 models) |

## Channels

| Channel | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| Telegram | Yes | Yes | Planned (via skill) | Yes |
| Slack | Yes | Yes | Planned (via skill) | Yes |
| Discord | Yes | Yes | Planned (via skill) | Yes |
| WhatsApp | Yes (Baileys) | Yes | Yes (Baileys, primary) | Tool only |
| Webhook (generic HTTP) | No | No | No | Yes |
| CLI / REPL | Yes | Yes | No | Yes |
| LINE | Yes | Yes | No | No |
| QQ | No | Yes | No | No |
| DingTalk | No | Yes | No | No |
| Feishu / Lark | No | Yes | No | No |
| Google Chat | Yes | No | No | No |
| Signal | Yes | No | No | No |
| iMessage | Yes | No | No | No |
| Microsoft Teams | Yes | No | No | No |
| Matrix | Yes | No | No | No |
| MaixCAM (device) | No | Yes | No | No |

OpenClaw leads on channel breadth (14+32 extensions). PicoClaw covers Chinese platforms (QQ, DingTalk, Feishu) and device channels. NanoClaw focuses on WhatsApp as its primary channel with others planned via community skills. ZeptoClaw has 5 channels plus a generic webhook.

## Security

| Feature | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| Workspace path isolation | Yes | Yes | Yes (per-group mount) | Yes |
| Shell command blocklist | Limited | Yes (8 patterns) | No (relies on container) | Yes (regex-based) |
| Prompt injection detection | No | No | No | Yes (17 patterns + 4 regex) |
| Secret leak scanning | No | No | No | Yes (22 patterns) |
| Security policy engine | No | No | No | Yes (7 rules) |
| Input validation | No | No | No | Yes (length, null bytes, repetition) |
| SSRF prevention | No | No | No | Yes (DNS pinning, private IP blocking) |
| Container isolation (per-agent) | No | No | Yes (Apple/Docker) | Yes (Docker + Apple Container) |
| Mount allowlist / blocklist | No | No | Yes (external allowlist) | Yes (mount policy) |
| Group/session isolation | Workspace-level | Workspace-level | Full filesystem + IPC | Per-request container |
| Tool approval gate | No | No | No | Yes |
| DM pairing / auth | Yes | No | No | No |
| Audit trails | Yes | No | No | No |

NanoClaw and ZeptoClaw both use OS-level container isolation (Apple Container + Docker) but apply it differently. NanoClaw isolates at the agent level — each group gets its own container with controlled mounts. ZeptoClaw isolates at the command level — each shell execution runs in a fresh container. Both block sensitive paths (.ssh, .aws, credentials).

**Known vulnerabilities:** OpenClaw's ecosystem has seen CVE-2026-25253 (CVSS 8.8 — WebSocket hijacking to RCE), ClawHavoc (341 malicious skills), and 42,000 exposed instances. PicoClaw, NanoClaw, and ZeptoClaw have no known CVEs as of this writing.

## Memory & Context

| Feature | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| Session persistence | Yes | Yes | Yes (SQLite) | Yes |
| Conversation history CLI | No | No | No | Yes (list, show, search, cleanup) |
| Long-term memory | Vector embeddings (SQLite) | Markdown files | Per-group CLAUDE.md | JSON KV store (categories, tags) |
| Memory search | Hybrid (keyword + vector) | Text search | No (file-based) | Fuzzy search + chunked scoring |
| Global shared memory | No | No | Yes (read-only global dir) | No |
| Token budget tracking | No | No | No | Yes (atomic per-session) |
| Context compaction | Yes (/compact) | No | No | Yes (auto-triggered) |
| Cost tracking | No | No | No | Yes (8 models, per-provider) |
| Metrics / telemetry | OpenTelemetry (extension) | No | No | Yes (Prometheus + JSON) |

## Architecture

| Aspect | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| Runtime | Node.js >= 22.12 | Go binary | Node.js 20+ | Rust binary (Tokio async) |
| Concurrency | Event loop | Goroutines | Per-group queue | Tokio async + futures::join_all |
| Agent execution | In-process | In-process | Container per group | In-process or container per command |
| Deployment | VPS, Docker Compose, Tailscale | Binary, Docker | Docker, Apple Container | Binary, Docker, systemd, one-click cloud |
| Companion apps | macOS, iOS, Android | No | No | No |
| Voice support | Wake + Talk Mode | Groq transcription | No | No |
| Multi-tenant | Yes (workspace isolation) | No | Yes (group isolation) | Yes (~6MB per tenant) |
| Config validation | Yes (doctor command) | Basic | No | Yes (config check) |
| Onboarding | Wizard | No | Claude Code skill | Interactive onboard |

## Design Philosophy

| | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| **Philosophy** | Feature-complete platform | Ultra-portable IoT agent | Minimal, forkable, secure | Dense, secure, agent-first |
| **Target user** | Power users, businesses | IoT/embedded developers | Single-user, personal | Developers, multi-tenant ops |
| **Extensibility** | Plugin ecosystem | None | Fork & modify code | Plugins + MCP + config |
| **Complexity** | High (~465K lines) | Low (~20.6K lines) | Minimal (~3.4K lines) | Moderate (~11K lines) |
| **Tagline** | "Personal AI assistant" | "AI for $10 hardware" | "Understand in 8 minutes" | "Ultra-lightweight personal AI assistant" |

## When to Use What

| Scenario | Recommended | Why |
|---|---|---|
| Maximum channel coverage | **OpenClaw** | 14 channels + 32 extensions, companion apps |
| Chinese platform integration | **PicoClaw** | QQ, DingTalk, Feishu built-in |
| Embedded / IoT devices | **PicoClaw** | I2C/SPI tools, <10MB RAM, $10 SBCs |
| WhatsApp-first personal assistant | **NanoClaw** | WhatsApp primary, group isolation, tiny codebase |
| Smallest codebase to fork | **NanoClaw** | 3.4K lines, designed to be forked |
| Security-sensitive deployment | **ZeptoClaw** | Multi-layer safety, container isolation, leak detection |
| Resource-constrained server | **ZeptoClaw** | 4MB binary, 6MB RAM, 50ms startup |
| Multi-tenant hosting | **ZeptoClaw** | ~6MB per tenant, container isolation per request |
| Plugin ecosystem | **OpenClaw** | 32 extensions, mature plugin system |
| Voice + mobile companion | **OpenClaw** | Wake Mode, Talk Mode, iOS/Android apps |
| Batch processing / automation | **ZeptoClaw** | Batch mode, routines, cron, agent templates |
| Cost-conscious API usage | **ZeptoClaw** | Token budget, cost tracking, retry + fallback |
| Browser automation | **OpenClaw** | CDP-based browser control |
| Hardware sensor integration | **PicoClaw** | I2C, SPI tools for SBCs |
| Agent swarms | **NanoClaw** or **ZeptoClaw** | NanoClaw: Claude Agent Teams. ZeptoClaw: DelegateTool with tool whitelists |

## Project Status

| | OpenClaw | PicoClaw | NanoClaw | ZeptoClaw |
|---|---|---|---|---|
| **Stage** | Established (160K+ stars) | New (5K+ stars in first week) | New | New (v0.3.1) |
| **Community** | Large | Growing fast | Small | Early |
| **Codebase** | ~465K lines TypeScript | ~20.6K lines Go | ~3.4K lines TypeScript | ~11K lines Rust |
| **Test coverage** | 949 test files | 222 tests | ~413 assertions | 1,314 tests |

---

**OpenClaw** is the most feature-rich — 52 skills, 14 channels, companion apps, voice support. The tradeoff is size (100MB+), resource usage, and security incidents (CVE-2026-25253, ClawHavoc supply chain attack).

**PicoClaw** is the most portable — Go binary on $10 RISC-V boards with I2C/SPI hardware tools and Chinese platform channels. The tradeoff is no security layer (no injection detection, no leak scanning, no container isolation).

**NanoClaw** is the most minimal — 3.4K lines of TypeScript designed to be forked and understood in 8 minutes. Full container isolation per group, WhatsApp-first. The tradeoff is Claude-only (no other LLM providers), one channel, and no built-in tools (relies on Claude Agent SDK).

**ZeptoClaw** is the most secure and resource-efficient — multi-layer safety, per-command container isolation, 4MB binary, 6MB RAM. The tradeoff is fewer channels (5 vs 14/10) and no companion apps.

All four are open source, self-hosted, and built for developers who want to own their AI assistant. The right choice depends on your priorities: features (OpenClaw), portability (PicoClaw), simplicity (NanoClaw), or security and efficiency (ZeptoClaw).
