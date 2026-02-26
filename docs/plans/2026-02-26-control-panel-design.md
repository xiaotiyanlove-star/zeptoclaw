# ZeptoClaw Control Panel — Design Document

**Date:** 2026-02-26
**Status:** Approved
**Approach:** Monorepo workspace (Approach B)

## Overview

A companion web dashboard for ZeptoClaw that provides real-time agent monitoring, cron/routine management, conversation viewing, kanban task board, and live agent visualization. Lives in the same repo as `panel/`, started via `zeptoclaw panel`.

**Audience:** ZeptoClaw users (developers)
**Deployment:** Local web app (localhost)
**Stack:** Vite + React + Tailwind (frontend), axum (API server in ZeptoClaw binary)
**Data layer:** REST API + WebSocket for real-time events

## Architecture

### Monorepo Structure

```
zeptoclaw/
├── Cargo.toml
├── src/
│   ├── cli/
│   │   └── panel.rs        # `zeptoclaw panel` command handler
│   ├── api/                 # axum API server module
│   │   ├── mod.rs
│   │   ├── server.rs        # axum router, static file serving, WebSocket
│   │   ├── auth.rs          # token + password auth middleware
│   │   ├── events.rs        # tokio::broadcast -> WebSocket bridge
│   │   └── routes/
│   │       ├── health.rs
│   │       ├── metrics.rs
│   │       ├── sessions.rs
│   │       ├── channels.rs
│   │       ├── cron.rs
│   │       ├── routines.rs
│   │       └── tasks.rs     # kanban board CRUD
│   └── ...
├── panel/                   # Vite + React + Tailwind
│   ├── package.json
│   ├── pnpm-lock.yaml
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   ├── index.html
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── pages/
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Logs.tsx
│   │   │   ├── Sessions.tsx
│   │   │   ├── CronRoutines.tsx
│   │   │   ├── Kanban.tsx
│   │   │   └── Agents.tsx
│   │   ├── components/
│   │   ├── hooks/
│   │   │   ├── useWebSocket.ts
│   │   │   ├── useHealth.ts
│   │   │   └── useAuth.ts
│   │   └── lib/
│   │       └── api.ts       # REST client
│   └── dist/                # built assets (gitignored)
└── ...
```

### CLI Commands

```bash
# One-command install
zeptoclaw panel install              # build from source (requires Node.js 18+)
zeptoclaw panel install --download   # download pre-built from GitHub releases (no Node.js)
zeptoclaw panel install --rebuild    # force clean rebuild

# Start
zeptoclaw panel                      # API + serve panel/dist/
zeptoclaw panel --dev                # API only, user runs pnpm dev separately
zeptoclaw panel --api-only           # headless API server
zeptoclaw panel --port 3000          # custom panel port
zeptoclaw panel --api-port 9091      # custom API port
zeptoclaw panel --bind 0.0.0.0      # external bind (requires TLS)

# Auth management
zeptoclaw panel auth mode password   # switch to password login
zeptoclaw panel auth mode token      # switch to token-only
zeptoclaw panel auth mode none       # disable auth
zeptoclaw panel auth reset-password  # reset password
zeptoclaw panel auth status          # show current mode

# Maintenance
zeptoclaw panel --rotate-token       # regenerate API token
zeptoclaw panel uninstall            # remove node_modules + dist + token
```

### Install Flow

`zeptoclaw panel install` performs:

1. Check `node` >= 18 on PATH (error with install link if missing)
2. Check `pnpm` (install via `corepack enable pnpm` if missing)
3. `pnpm install --dir panel/`
4. `pnpm --dir panel build` (outputs to `panel/dist/`)
5. Generate random 32-byte hex API token -> `~/.zeptoclaw/panel.token`
6. Interactive auth setup prompt (token / password / skip)
7. Print success message

For crates.io installs (`--download`):
- Fetch pre-built `panel-dist.tar.gz` from GitHub release matching binary version
- Extract to `~/.zeptoclaw/panel/dist/`
- No Node.js required

Panel location resolution order:
1. `./panel/dist/` (repo checkout)
2. `~/.zeptoclaw/panel/dist/` (downloaded)
3. Error with install instructions

## API Design

Base: `http://localhost:9091/api`

### REST Endpoints

```
GET  /health                    # system health (version, uptime, RSS, components)
GET  /metrics                   # telemetry + cost breakdown
GET  /csrf-token                # CSRF token for mutating requests

GET  /sessions                  # list sessions (key, msg count, last active)
GET  /sessions/:key             # full conversation
DEL  /sessions/:key             # delete session

GET  /channels                  # channel status (name, up/down, restart count)

GET  /cron                      # list cron jobs with state
POST /cron                      # create job
PUT  /cron/:id                  # update job
DEL  /cron/:id                  # delete job
POST /cron/:id/trigger          # manual trigger

GET  /routines                  # list routines
POST /routines                  # create routine
PUT  /routines/:id              # update routine
DEL  /routines/:id              # delete routine
POST /routines/:id/toggle       # enable/disable

GET  /tasks                     # list tasks (filterable by status/assignee)
POST /tasks                     # create task
PUT  /tasks/:id                 # update task
DEL  /tasks/:id                 # delete task
POST /tasks/:id/move            # move between kanban columns

WS   /ws/events                 # real-time event stream
```

### WebSocket Events

```json
{"type": "tool_started", "tool": "web_search", "ts": 1709000000}
{"type": "tool_done", "tool": "web_search", "duration_ms": 230, "ts": 1709000000}
{"type": "tool_failed", "tool": "shell", "error": "timeout", "ts": 1709000000}
{"type": "message_received", "channel": "telegram", "chat_id": "123", "ts": 1709000000}
{"type": "agent_started", "session_key": "telegram:123", "ts": 1709000000}
{"type": "agent_done", "session_key": "telegram:123", "tokens": 1500, "ts": 1709000000}
{"type": "compaction", "from_tokens": 80000, "to_tokens": 30000, "ts": 1709000000}
{"type": "channel_status", "channel": "telegram", "status": "up", "ts": 1709000000}
{"type": "cron_fired", "job_id": "abc", "status": "ok", "ts": 1709000000}
```

### Kanban Task Model

New data model persisted at `~/.zeptoclaw/tasks.json`:

```json
{
  "id": "uuid",
  "title": "Implement webhook retry",
  "description": "Add exponential backoff...",
  "column": "in_progress",
  "assignee": "agent",
  "priority": "high",
  "labels": ["feat", "area:channels"],
  "created_at": "2026-02-26T...",
  "updated_at": "2026-02-26T..."
}
```

Columns: `backlog`, `in_progress`, `review`, `done`

Agent access: new `TaskTool` in `src/tools/` exposes create/update/move/list actions.

## Frontend Pages

### 1. Dashboard
- Health status pill (green/yellow/red)
- Uptime, memory RSS, binary version
- Channel status cards with restart counts
- Token usage (input/output) + estimated cost
- Active sessions count
- Mini activity feed (last 10 WebSocket events)

### 2. Logs
- Live-scrolling event log from WebSocket
- Filter by event type, channel, session
- Pause/resume stream
- Expandable rows for tool errors
- Color-coded by type (green/red/blue)

### 3. Sessions
- Session list with search/filter
- Chat bubble conversation viewer (user/assistant/tool)
- Tool calls as collapsible blocks
- Per-session stats (messages, tokens, duration, cost)

### 4. Cron & Routines (tabbed)
- Cron: job list, next run, last status, error backoff indicator
- Create/edit with cron expression builder
- Routines: trigger type badges, enable/disable toggles
- Manual trigger button
- Execution history timeline

### 5. Kanban
- 4 columns: Backlog, In Progress, Review, Done
- Drag-and-drop cards
- Cards show: title, assignee (human/agent), priority, labels
- Agent can create/move cards via TaskTool
- Filter by assignee, label, priority

### 6. Agents (Live Office)
- Visual grid of active agent "desks"
- Each desk: channel icon, current tool, token meter
- Idle agents dimmed, active animated
- Click desk -> jump to session logs
- Swarm view: parent->child tree when DelegateTool fires

### Frontend Libraries
- `@tanstack/react-query` — REST data fetching + cache
- `react-router` — page navigation
- `@dnd-kit/core` — kanban drag-drop
- `recharts` — dashboard charts
- `tailwindcss` — styling
- Native `WebSocket` + React context — real-time events

## Security Model

### Authentication (3 modes)

| Mode | Login screen | API token | Best for |
|------|-------------|-----------|----------|
| `token` (default) | No | Auto-injected | Solo dev, localhost |
| `password` | Yes | Issued after login | Shared network, Tailscale |
| `none` | No | No | Trusted LAN, quick testing |

**Token mode:** API token from `~/.zeptoclaw/panel.token`, injected via one-time httpOnly cookie (5s expiry).

**Password mode:**
- Login page at `/login`
- Bcrypt hash stored in `~/.zeptoclaw/panel.auth`
- JWT (HS256, 24h expiry) issued on success, stored as httpOnly cookie
- 5 failed attempts -> 60s lockout

**Auth file** (`~/.zeptoclaw/panel.auth`, chmod 600):
```json
{
  "username": "admin",
  "password_hash": "$2b$12$...",
  "created_at": "2026-02-26T...",
  "updated_at": "2026-02-26T..."
}
```

### CORS & CSRF
- Strict CORS: only `http://localhost:<panel-port>`
- No wildcard origins
- All POST/PUT/DELETE require `X-CSRF-Token` header
- WebSocket upgrade validates token on connect

### Input Validation & Rate Limiting
- Request body size limit: 1MB
- Cron expressions validated server-side
- Task/routine names sanitized (no XSS)
- Rate limit: 100 req/s via `tower::limit`
- WebSocket connection cap: 5 concurrent

### Data Exposure Controls
- Session tool results redacted by default (opt-in full payloads)
- API never exposes: API keys, bot tokens, master key, OAuth tokens
- Log events stripped of sensitive args (reuses `src/utils/sanitize.rs`)
- Config endpoint (if added) shows non-secret fields only

### Network Binding
- Default: `127.0.0.1` only
- `--bind 0.0.0.0` requires `--tls-cert` + `--tls-key` (refuses without)
- Warning printed when binding externally

### Config

```json
{
  "panel": {
    "enabled": true,
    "port": 9092,
    "api_port": 9091,
    "auth_mode": "token",
    "bind": "127.0.0.1"
  }
}
```

## What Changes in ZeptoClaw Core

1. **New module: `src/api/`** — axum server with routes, auth middleware, WebSocket
2. **New module: `src/cli/panel.rs`** — `panel` command (install, start, auth subcommands)
3. **New tool: `src/tools/task.rs`** — TaskTool for kanban CRUD (agent-accessible)
4. **New data: `~/.zeptoclaw/tasks.json`** — kanban task persistence
5. **Event bus: `src/bus/`** — tokio::broadcast channel for agent loop events -> WebSocket
6. **Agent loop instrumentation** — emit events to bus (tool start/done/fail, agent start/done)
7. **New deps in Cargo.toml:** `axum`, `tower-http` (cors, static files), `jsonwebtoken`, `bcrypt`
8. **Config additions:** `PanelConfig` struct in `src/config/types.rs`

## Binary Size Impact

- `axum` + `tower-http`: ~300KB
- `jsonwebtoken`: ~50KB
- `bcrypt`: ~100KB
- Total: ~450KB (4MB -> ~4.5MB)

Panel assets are NOT embedded — served from disk at runtime.
