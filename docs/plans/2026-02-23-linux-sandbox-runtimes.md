# Linux Sandbox Runtimes Design

**Date:** 2026-02-23
**Status:** Approved
**Scope:** Add Landlock, Firejail, and Bubblewrap as optional, configurable `RuntimeType` variants

---

## Background

ZeptoClaw currently has three shell execution runtimes: `Native`, `Docker`, and `AppleContainer`. The shell security layer uses a **blocklist** (`src/security/shell.rs`) which, by design, cannot be exhaustive. Container isolation (Docker/Apple) provides the primary sandbox boundary on those platforms.

On Linux, three additional lightweight sandbox tools are available that complement or replace container-level isolation:

- **Landlock** — Linux kernel LSM (5.13+). Pure-Rust via the `landlock` crate. Restricts filesystem and network access at the syscall level with no binary dependency.
- **Firejail** — Userspace sandbox wrapper using Linux namespaces and seccomp. Requires `firejail` binary. Profile-based configuration.
- **Bubblewrap** — Lightweight OCI-compatible sandbox (`bwrap` binary). Used by Flatpak. Fine-grained bind-mount control.

This design adds all three as first-class `RuntimeType` variants following the existing runtime pattern, plus an optional command **allowlist** to complement the existing blocklist.

---

## Approach: Three Independent RuntimeType Variants

Each sandbox is a separate `RuntimeType` variant with its own:
- Runtime struct implementing `ContainerRuntime` trait
- Config struct in `config/types.rs`
- Cargo feature flag (independently opt-in)
- `#[cfg(target_os = "linux")]` platform guard

This matches the existing `Docker` / `AppleContainer` pattern exactly.

---

## New Files

```
src/runtime/
├── landlock.rs      # LandlockRuntime — kernel LSM via landlock crate
├── firejail.rs      # FirejailRuntime — wraps firejail binary
└── bubblewrap.rs    # BubblewrapRuntime — wraps bwrap binary
```

---

## RuntimeType Enum Changes

```rust
pub enum RuntimeType {
    Native,
    Docker,
    #[serde(rename = "apple")]
    AppleContainer,
    // New — Linux only
    #[cfg_attr(not(target_os = "linux"), serde(skip))]
    Landlock,
    #[cfg_attr(not(target_os = "linux"), serde(skip))]
    Firejail,
    #[cfg_attr(not(target_os = "linux"), serde(skip))]
    Bubblewrap,
}
```

---

## Runtime Implementations

### LandlockRuntime (`src/runtime/landlock.rs`)

Uses the `landlock` crate to apply filesystem access rules to the child process before exec. Gracefully degrades on kernels < 5.13 (ABI negotiation built into the crate).

```rust
pub struct LandlockRuntime {
    config: LandlockConfig,
}
```

Execution flow:
1. Build `Ruleset` with `AccessFs` rules from `LandlockConfig`
2. Spawn `sh -c <command>` via `tokio::process::Command`
3. Apply Landlock restrictions in the child process via `pre_exec` hook
4. Capture stdout/stderr with timeout

### FirejailRuntime (`src/runtime/firejail.rs`)

Wraps command as: `firejail [--profile=<path>] --noprofile sh -c <command>`

```rust
pub struct FirejailRuntime {
    config: FirejailConfig,
}
```

`is_available()` checks `which firejail` on PATH.

### BubblewrapRuntime (`src/runtime/bubblewrap.rs`)

Wraps command as:
```
bwrap
  --ro-bind /usr /usr
  --ro-bind /lib /lib
  --ro-bind /etc /etc
  --dev /dev
  --proc /proc
  --bind <workspace> <workspace>
  [extra_args...]
  sh -c <command>
```

```rust
pub struct BubblewrapRuntime {
    config: BubblewrapConfig,
}
```

`is_available()` checks `which bwrap` on PATH.

---

## Config Additions (`src/config/types.rs`)

### RuntimeConfig (extended)

```rust
pub struct RuntimeConfig {
    pub runtime_type: RuntimeType,
    pub allow_fallback_to_native: bool,
    pub mount_allowlist_path: String,
    pub docker: DockerConfig,
    pub apple: AppleContainerConfig,
    // New:
    pub landlock: LandlockConfig,
    pub firejail: FirejailConfig,
    pub bubblewrap: BubblewrapConfig,
}
```

### LandlockConfig

```rust
pub struct LandlockConfig {
    /// Directories the sandboxed process may read
    pub fs_read_dirs: Vec<String>,
    /// Directories the sandboxed process may write
    pub fs_write_dirs: Vec<String>,
    /// Automatically add workspace root to read+write allow list
    pub allow_read_workspace: bool,
    pub allow_write_workspace: bool,
}
```

Default read dirs: `["/usr", "/lib", "/lib64", "/etc", "/bin", "/sbin", "/tmp"]`
Default write dirs: `["/tmp"]`

### FirejailConfig

```rust
pub struct FirejailConfig {
    /// Path to a custom firejail profile file (None = --noprofile)
    pub profile: Option<String>,
    /// Extra firejail arguments passed verbatim
    pub extra_args: Vec<String>,
}
```

### BubblewrapConfig

```rust
pub struct BubblewrapConfig {
    /// Read-only bind mounts (host path = container path)
    pub ro_binds: Vec<String>,
    /// Bind /dev (needed for most commands)
    pub dev_bind: bool,
    /// Bind /proc
    pub proc_bind: bool,
    /// Extra bwrap arguments passed verbatim
    pub extra_args: Vec<String>,
}
```

Default `ro_binds`: `["/usr", "/lib", "/lib64", "/etc", "/bin", "/sbin"]`

---

## Command Allowlist (`src/security/shell.rs` + `SecurityConfig`)

New config field:

```rust
pub struct SecurityConfig {
    // existing fields...
    pub shell_allowlist: Vec<String>,
    pub shell_allowlist_mode: ShellAllowlistMode,
}

pub enum ShellAllowlistMode {
    Off,    // default — blocklist only (current behaviour)
    Warn,   // log if command not in allowlist, but proceed
    Strict, // block if first token not in allowlist
}
```

Example config:
```json
{
  "security": {
    "shell_allowlist": ["git", "cargo", "python3", "ls", "cat", "echo"],
    "shell_allowlist_mode": "strict"
  }
}
```

`check_shell_command()` runs blocklist first, then allowlist check if mode != Off.

---

## Cargo Features

```toml
# Linux sandbox runtimes (all Linux-only)
sandbox-landlock   = ["dep:landlock"]    # landlock crate (~1 crate, no binaries)
sandbox-firejail   = []                  # no extra deps — wraps firejail binary
sandbox-bubblewrap = []                  # no extra deps — wraps bwrap binary
```

`[dependencies]`
```toml
landlock = { version = "0.4", optional = true }
```

---

## Factory Changes (`src/runtime/factory.rs`)

New match arms in `create_runtime()`:

```rust
RuntimeType::Landlock => { /* feature check, linux check, build LandlockRuntime */ }
RuntimeType::Firejail => { /* feature check, linux check, binary check, build FirejailRuntime */ }
RuntimeType::Bubblewrap => { /* feature check, linux check, binary check, build BubblewrapRuntime */ }
```

`available_runtimes()` extended to probe for each.

---

## Platform Behaviour

| Runtime    | macOS | Linux (feature off) | Linux (feature on) |
|------------|-------|---------------------|--------------------|
| Landlock   | Error: requires Linux | Error: recompile with `--features sandbox-landlock` | Available if kernel ≥ 5.13 |
| Firejail   | Error: requires Linux | Error: recompile with `--features sandbox-firejail` | Available if `firejail` on PATH |
| Bubblewrap | Error: requires Linux | Error: recompile with `--features sandbox-bubblewrap` | Available if `bwrap` on PATH |

---

## Testing

- Unit tests per runtime: `is_available()`, `execute()` basic echo, workdir, timeout
- Landlock: test filesystem restriction (attempt to read `/etc/passwd` outside allowlist → error)
- Firejail/Bubblewrap: integration tests gated behind feature + binary availability check
- Allowlist: unit tests for Off/Warn/Strict modes, first-token extraction
- All tests compile-guarded with `#[cfg(target_os = "linux")]` and `#[cfg(feature = "sandbox-*")]`

---

## CLAUDE.md / AGENTS.md Updates

- Add three new `RuntimeType` variants to architecture section
- Document new Cargo features
- Add env vars: `ZEPTOCLAW_RUNTIME_TYPE=landlock|firejail|bubblewrap`
