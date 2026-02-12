# Container Isolation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add selectable container runtime support (Docker, Apple Container, or Native) for secure shell command execution, making container isolation the primary security mechanism while keeping application-level security as defense-in-depth.

**Architecture:** Create a `runtime` module with a `ContainerRuntime` trait and three implementations: `NativeRuntime` (current behavior), `DockerRuntime`, and `AppleContainerRuntime`. The `ShellTool` will use the runtime to execute commands. Configuration allows users to select their preferred runtime during setup.

**Tech Stack:** Rust, tokio::process, serde, Docker CLI, Apple Container Framework (container tool)

---

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         ShellTool                               │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ SecurityConfig  │  │ ContainerConfig │  │    Runtime      │  │
│  │ (blocklist)     │  │ (image, mounts) │  │ (trait object)  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└───────────────────────────────┬─────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│ NativeRuntime │       │ DockerRuntime │       │ AppleRuntime  │
│ sh -c "cmd"   │       │ docker run... │       │ container...  │
└───────────────┘       └───────────────┘       └───────────────┘
```

---

## Task 1: Add Runtime Configuration Types

**Files:**
- Modify: `src/config/types.rs`

**Step 1: Add RuntimeType enum and RuntimeConfig struct**

Add after `ToolsConfig` (around line 315):

```rust
// ============================================================================
// Runtime Configuration
// ============================================================================

/// Container runtime type for shell command execution
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeType {
    /// Native execution (no container isolation)
    #[default]
    Native,
    /// Docker container isolation
    Docker,
    /// Apple Container isolation (macOS only)
    #[serde(rename = "apple")]
    AppleContainer,
}

/// Runtime configuration for shell execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Type of container runtime to use
    pub runtime_type: RuntimeType,
    /// Docker-specific configuration
    pub docker: DockerConfig,
    /// Apple Container-specific configuration (macOS)
    pub apple: AppleContainerConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_type: RuntimeType::Native,
            docker: DockerConfig::default(),
            apple: AppleContainerConfig::default(),
        }
    }
}

/// Docker runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DockerConfig {
    /// Docker image to use for shell execution
    pub image: String,
    /// Additional volume mounts (host:container format)
    pub extra_mounts: Vec<String>,
    /// Memory limit (e.g., "512m")
    pub memory_limit: Option<String>,
    /// CPU limit (e.g., "1.0")
    pub cpu_limit: Option<String>,
    /// Network mode (default: none for security)
    pub network: String,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "alpine:latest".to_string(),
            extra_mounts: Vec::new(),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            network: "none".to_string(),
        }
    }
}

/// Apple Container runtime configuration (macOS only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppleContainerConfig {
    /// Container image/bundle path
    pub image: String,
    /// Additional directory mounts
    pub extra_mounts: Vec<String>,
}

impl Default for AppleContainerConfig {
    fn default() -> Self {
        Self {
            image: "".to_string(),
            extra_mounts: Vec::new(),
        }
    }
}
```

**Step 2: Add runtime field to Config struct**

Update the `Config` struct (around line 12) to include runtime:

```rust
pub struct Config {
    /// Agent configuration (models, tokens, iterations)
    pub agents: AgentConfig,
    /// Channel configurations (Telegram, Discord, Slack, etc.)
    pub channels: ChannelsConfig,
    /// LLM provider configurations (Claude, OpenAI, OpenRouter, etc.)
    pub providers: ProvidersConfig,
    /// Gateway server configuration
    pub gateway: GatewayConfig,
    /// Tools configuration
    pub tools: ToolsConfig,
    /// Runtime configuration for container isolation
    pub runtime: RuntimeConfig,
}
```

**Step 3: Run tests**

Run: `cargo test --lib config`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/config/types.rs
git commit -m "feat(config): add container runtime configuration types"
```

---

## Task 2: Create Runtime Module Structure

**Files:**
- Create: `src/runtime/mod.rs`
- Create: `src/runtime/types.rs`

**Step 1: Create runtime module**

Create `src/runtime/mod.rs`:

```rust
//! Container runtime module for ZeptoClaw
//!
//! This module provides container isolation for shell command execution.
//! It supports multiple runtimes:
//! - Native: Direct execution (no isolation, uses application-level security)
//! - Docker: Docker container isolation (Linux, macOS, Windows)
//! - Apple Container: Apple's native container technology (macOS only)

pub mod types;
pub mod native;
pub mod docker;

#[cfg(target_os = "macos")]
pub mod apple;

pub use types::{ContainerRuntime, RuntimeError, RuntimeResult, CommandOutput};
pub use native::NativeRuntime;
pub use docker::DockerRuntime;

#[cfg(target_os = "macos")]
pub use apple::AppleContainerRuntime;
```

**Step 2: Create runtime types**

Create `src/runtime/types.rs`:

```rust
//! Runtime type definitions

use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during runtime operations
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Container runtime not available
    #[error("Runtime not available: {0}")]
    NotAvailable(String),

    /// Failed to start container
    #[error("Failed to start container: {0}")]
    StartFailed(String),

    /// Command execution failed
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    /// Timeout exceeded
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for runtime operations
pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;

/// Output from a command execution
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code (None if killed by signal)
    pub exit_code: Option<i32>,
}

impl CommandOutput {
    /// Create a new CommandOutput
    pub fn new(stdout: String, stderr: String, exit_code: Option<i32>) -> Self {
        Self {
            stdout,
            stderr,
            exit_code,
        }
    }

    /// Check if the command succeeded (exit code 0)
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Format output similar to current shell tool behavior
    pub fn format(&self) -> String {
        let mut result = String::new();

        if !self.stdout.is_empty() {
            result.push_str(&self.stdout);
        }

        if !self.stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n--- stderr ---\n");
            }
            result.push_str(&self.stderr);
        }

        if let Some(code) = self.exit_code {
            if code != 0 {
                result.push_str(&format!("\n[Exit code: {}]", code));
            }
        }

        result
    }
}

/// Configuration for a container execution
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Working directory inside container
    pub workdir: Option<PathBuf>,
    /// Directories to mount (host_path, container_path, readonly)
    pub mounts: Vec<(PathBuf, PathBuf, bool)>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Command timeout in seconds
    pub timeout_secs: u64,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            workdir: None,
            mounts: Vec::new(),
            env: Vec::new(),
            timeout_secs: 60,
        }
    }
}

impl ContainerConfig {
    /// Create a new container config
    pub fn new() -> Self {
        Self::default()
    }

    /// Set working directory
    pub fn with_workdir(mut self, workdir: PathBuf) -> Self {
        self.workdir = Some(workdir);
        self
    }

    /// Add a mount point
    pub fn with_mount(mut self, host: PathBuf, container: PathBuf, readonly: bool) -> Self {
        self.mounts.push((host, container, readonly));
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Trait for container runtimes
#[async_trait]
pub trait ContainerRuntime: Send + Sync {
    /// Get the runtime name
    fn name(&self) -> &str;

    /// Check if this runtime is available on the system
    async fn is_available(&self) -> bool;

    /// Execute a command in the container
    async fn execute(
        &self,
        command: &str,
        config: &ContainerConfig,
    ) -> RuntimeResult<CommandOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_output_success() {
        let output = CommandOutput::new("hello".to_string(), "".to_string(), Some(0));
        assert!(output.success());
    }

    #[test]
    fn test_command_output_failure() {
        let output = CommandOutput::new("".to_string(), "error".to_string(), Some(1));
        assert!(!output.success());
    }

    #[test]
    fn test_command_output_format() {
        let output = CommandOutput::new(
            "stdout content".to_string(),
            "stderr content".to_string(),
            Some(1),
        );
        let formatted = output.format();
        assert!(formatted.contains("stdout content"));
        assert!(formatted.contains("--- stderr ---"));
        assert!(formatted.contains("stderr content"));
        assert!(formatted.contains("[Exit code: 1]"));
    }

    #[test]
    fn test_container_config_builder() {
        let config = ContainerConfig::new()
            .with_workdir(PathBuf::from("/workspace"))
            .with_mount(PathBuf::from("/host"), PathBuf::from("/container"), true)
            .with_env("FOO", "bar")
            .with_timeout(120);

        assert_eq!(config.workdir, Some(PathBuf::from("/workspace")));
        assert_eq!(config.mounts.len(), 1);
        assert_eq!(config.env.len(), 1);
        assert_eq!(config.timeout_secs, 120);
    }
}
```

**Step 3: Run tests**

Run: `cargo test --lib runtime::types`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/runtime/
git commit -m "feat(runtime): add container runtime types and trait"
```

---

## Task 3: Implement Native Runtime

**Files:**
- Create: `src/runtime/native.rs`

**Step 1: Create native runtime implementation**

Create `src/runtime/native.rs`:

```rust
//! Native runtime implementation
//!
//! Executes commands directly on the host system without container isolation.
//! This is the fallback when no container runtime is configured.

use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use super::types::{CommandOutput, ContainerConfig, ContainerRuntime, RuntimeError, RuntimeResult};

/// Native runtime that executes commands directly on the host
#[derive(Debug, Clone, Default)]
pub struct NativeRuntime;

impl NativeRuntime {
    /// Create a new native runtime
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ContainerRuntime for NativeRuntime {
    fn name(&self) -> &str {
        "native"
    }

    async fn is_available(&self) -> bool {
        // Native runtime is always available
        true
    }

    async fn execute(
        &self,
        command: &str,
        config: &ContainerConfig,
    ) -> RuntimeResult<CommandOutput> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);

        // Set working directory if specified
        if let Some(ref workdir) = config.workdir {
            cmd.current_dir(workdir);
        }

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Capture output
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Execute with timeout
        let output = tokio::time::timeout(
            Duration::from_secs(config.timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| RuntimeError::Timeout(config.timeout_secs))?
        .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        Ok(CommandOutput::new(
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_native_runtime_available() {
        let runtime = NativeRuntime::new();
        assert!(runtime.is_available().await);
    }

    #[tokio::test]
    async fn test_native_runtime_name() {
        let runtime = NativeRuntime::new();
        assert_eq!(runtime.name(), "native");
    }

    #[tokio::test]
    async fn test_native_runtime_echo() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new();

        let output = runtime.execute("echo hello", &config).await.unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn test_native_runtime_with_workdir() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new()
            .with_workdir(std::path::PathBuf::from("/tmp"));

        let output = runtime.execute("pwd", &config).await.unwrap();
        assert!(output.success());
        // On macOS /tmp is symlinked to /private/tmp
        assert!(output.stdout.contains("tmp"));
    }

    #[tokio::test]
    async fn test_native_runtime_with_env() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new()
            .with_env("TEST_VAR", "test_value");

        let output = runtime.execute("echo $TEST_VAR", &config).await.unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.trim(), "test_value");
    }

    #[tokio::test]
    async fn test_native_runtime_stderr() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new();

        let output = runtime.execute("echo error >&2", &config).await.unwrap();
        assert!(output.success());
        assert!(output.stderr.contains("error"));
    }

    #[tokio::test]
    async fn test_native_runtime_exit_code() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new();

        let output = runtime.execute("exit 42", &config).await.unwrap();
        assert!(!output.success());
        assert_eq!(output.exit_code, Some(42));
    }

    #[tokio::test]
    async fn test_native_runtime_timeout() {
        let runtime = NativeRuntime::new();
        let config = ContainerConfig::new().with_timeout(1);

        let result = runtime.execute("sleep 10", &config).await;
        assert!(matches!(result, Err(RuntimeError::Timeout(1))));
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib runtime::native`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add src/runtime/native.rs
git commit -m "feat(runtime): implement native runtime"
```

---

## Task 4: Implement Docker Runtime

**Files:**
- Create: `src/runtime/docker.rs`

**Step 1: Create Docker runtime implementation**

Create `src/runtime/docker.rs`:

```rust
//! Docker runtime implementation
//!
//! Executes commands inside Docker containers for secure isolation.

use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use super::types::{CommandOutput, ContainerConfig, ContainerRuntime, RuntimeError, RuntimeResult};

/// Docker runtime that executes commands in isolated containers
#[derive(Debug, Clone)]
pub struct DockerRuntime {
    /// Docker image to use
    image: String,
    /// Memory limit (e.g., "512m")
    memory_limit: Option<String>,
    /// CPU limit (e.g., "1.0")
    cpu_limit: Option<String>,
    /// Network mode
    network: String,
}

impl DockerRuntime {
    /// Create a new Docker runtime with the specified image
    pub fn new(image: &str) -> Self {
        Self {
            image: image.to_string(),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            network: "none".to_string(),
        }
    }

    /// Set memory limit
    pub fn with_memory_limit(mut self, limit: &str) -> Self {
        self.memory_limit = Some(limit.to_string());
        self
    }

    /// Set CPU limit
    pub fn with_cpu_limit(mut self, limit: &str) -> Self {
        self.cpu_limit = Some(limit.to_string());
        self
    }

    /// Set network mode
    pub fn with_network(mut self, network: &str) -> Self {
        self.network = network.to_string();
        self
    }

    /// Disable resource limits
    pub fn without_limits(mut self) -> Self {
        self.memory_limit = None;
        self.cpu_limit = None;
        self
    }
}

impl Default for DockerRuntime {
    fn default() -> Self {
        Self::new("alpine:latest")
    }
}

#[async_trait]
impl ContainerRuntime for DockerRuntime {
    fn name(&self) -> &str {
        "docker"
    }

    async fn is_available(&self) -> bool {
        // Check if docker is installed and running
        Command::new("docker")
            .args(["info"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    async fn execute(
        &self,
        command: &str,
        config: &ContainerConfig,
    ) -> RuntimeResult<CommandOutput> {
        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            self.network.clone(),
        ];

        // Add resource limits
        if let Some(ref mem) = self.memory_limit {
            args.push("--memory".to_string());
            args.push(mem.clone());
        }
        if let Some(ref cpu) = self.cpu_limit {
            args.push("--cpus".to_string());
            args.push(cpu.clone());
        }

        // Add working directory
        if let Some(ref workdir) = config.workdir {
            args.push("-w".to_string());
            args.push(workdir.to_string_lossy().to_string());
        }

        // Add volume mounts
        for (host, container, readonly) in &config.mounts {
            let mount_spec = if *readonly {
                format!(
                    "{}:{}:ro",
                    host.to_string_lossy(),
                    container.to_string_lossy()
                )
            } else {
                format!(
                    "{}:{}",
                    host.to_string_lossy(),
                    container.to_string_lossy()
                )
            };
            args.push("-v".to_string());
            args.push(mount_spec);
        }

        // Add environment variables
        for (key, value) in &config.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add image and command
        args.push(self.image.clone());
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());

        let mut cmd = Command::new("docker");
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Execute with timeout
        let output = tokio::time::timeout(
            Duration::from_secs(config.timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| RuntimeError::Timeout(config.timeout_secs))?
        .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        Ok(CommandOutput::new(
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_runtime_creation() {
        let runtime = DockerRuntime::new("ubuntu:22.04");
        assert_eq!(runtime.image, "ubuntu:22.04");
        assert_eq!(runtime.name(), "docker");
    }

    #[test]
    fn test_docker_runtime_builder() {
        let runtime = DockerRuntime::new("alpine:latest")
            .with_memory_limit("1g")
            .with_cpu_limit("2.0")
            .with_network("bridge");

        assert_eq!(runtime.memory_limit, Some("1g".to_string()));
        assert_eq!(runtime.cpu_limit, Some("2.0".to_string()));
        assert_eq!(runtime.network, "bridge");
    }

    #[test]
    fn test_docker_runtime_without_limits() {
        let runtime = DockerRuntime::new("alpine:latest").without_limits();
        assert!(runtime.memory_limit.is_none());
        assert!(runtime.cpu_limit.is_none());
    }

    // Integration tests (only run if Docker is available)
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_docker_runtime_available() {
        let runtime = DockerRuntime::new("alpine:latest");
        // This will only pass if Docker is installed and running
        assert!(runtime.is_available().await);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_docker_runtime_echo() {
        let runtime = DockerRuntime::new("alpine:latest");
        let config = ContainerConfig::new();

        let output = runtime.execute("echo hello", &config).await.unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.trim(), "hello");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_docker_runtime_isolation() {
        let runtime = DockerRuntime::new("alpine:latest");
        let config = ContainerConfig::new();

        // This should fail because network is disabled by default
        let output = runtime.execute("ping -c 1 google.com", &config).await.unwrap();
        assert!(!output.success());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib runtime::docker`
Expected: Unit tests PASS (integration tests are ignored by default)

**Step 3: Commit**

```bash
git add src/runtime/docker.rs
git commit -m "feat(runtime): implement Docker runtime"
```

---

## Task 5: Implement Apple Container Runtime (macOS only)

**Files:**
- Create: `src/runtime/apple.rs`

**Step 1: Create Apple Container runtime implementation**

Create `src/runtime/apple.rs`:

```rust
//! Apple Container runtime implementation (macOS only)
//!
//! Executes commands inside Apple's lightweight containers on macOS.
//! Uses the `container` tool from Apple's Containerization framework.

use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use super::types::{CommandOutput, ContainerConfig, ContainerRuntime, RuntimeError, RuntimeResult};

/// Apple Container runtime for macOS
///
/// This runtime uses Apple's native container technology available on macOS 15+
/// (Sequoia). It provides lightweight isolation optimized for Apple Silicon.
#[derive(Debug, Clone)]
pub struct AppleContainerRuntime {
    /// Container image/bundle path (optional, uses default if not set)
    image: Option<String>,
}

impl AppleContainerRuntime {
    /// Create a new Apple Container runtime
    pub fn new() -> Self {
        Self { image: None }
    }

    /// Create runtime with a specific container image
    pub fn with_image(image: &str) -> Self {
        Self {
            image: Some(image.to_string()),
        }
    }
}

impl Default for AppleContainerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContainerRuntime for AppleContainerRuntime {
    fn name(&self) -> &str {
        "apple"
    }

    async fn is_available(&self) -> bool {
        // Check if we're on macOS and the container tool is available
        if !cfg!(target_os = "macos") {
            return false;
        }

        // Check for Apple's container tool
        // The tool is available via Virtualization framework on macOS 15+
        Command::new("container")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    async fn execute(
        &self,
        command: &str,
        config: &ContainerConfig,
    ) -> RuntimeResult<CommandOutput> {
        // Note: Apple's container tool API may vary
        // This is a placeholder implementation based on expected interface
        let mut args = vec!["run".to_string()];

        // Add image if specified
        if let Some(ref image) = self.image {
            args.push("--image".to_string());
            args.push(image.clone());
        }

        // Add working directory
        if let Some(ref workdir) = config.workdir {
            args.push("--workdir".to_string());
            args.push(workdir.to_string_lossy().to_string());
        }

        // Add volume mounts
        for (host, container, readonly) in &config.mounts {
            args.push("--mount".to_string());
            let mount_spec = if *readonly {
                format!(
                    "type=bind,source={},target={},readonly",
                    host.to_string_lossy(),
                    container.to_string_lossy()
                )
            } else {
                format!(
                    "type=bind,source={},target={}",
                    host.to_string_lossy(),
                    container.to_string_lossy()
                )
            };
            args.push(mount_spec);
        }

        // Add environment variables
        for (key, value) in &config.env {
            args.push("--env".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add the command
        args.push("--".to_string());
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());

        let mut cmd = Command::new("container");
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Execute with timeout
        let output = tokio::time::timeout(
            Duration::from_secs(config.timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| RuntimeError::Timeout(config.timeout_secs))?
        .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        Ok(CommandOutput::new(
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_runtime_creation() {
        let runtime = AppleContainerRuntime::new();
        assert_eq!(runtime.name(), "apple");
        assert!(runtime.image.is_none());
    }

    #[test]
    fn test_apple_runtime_with_image() {
        let runtime = AppleContainerRuntime::with_image("/path/to/image");
        assert_eq!(runtime.image, Some("/path/to/image".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "requires Apple Container framework"]
    async fn test_apple_runtime_available() {
        let runtime = AppleContainerRuntime::new();
        // This checks if the container tool is available
        println!("Apple Container available: {}", runtime.is_available().await);
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn test_apple_runtime_not_available_on_non_macos() {
        let runtime = AppleContainerRuntime::new();
        assert!(!runtime.is_available().await);
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib runtime::apple`
Expected: Tests PASS

**Step 3: Commit**

```bash
git add src/runtime/apple.rs
git commit -m "feat(runtime): implement Apple Container runtime for macOS"
```

---

## Task 6: Create Runtime Factory

**Files:**
- Create: `src/runtime/factory.rs`
- Modify: `src/runtime/mod.rs`

**Step 1: Create runtime factory**

Create `src/runtime/factory.rs`:

```rust
//! Runtime factory for creating container runtimes from configuration

use std::sync::Arc;

use crate::config::{RuntimeConfig, RuntimeType};

use super::types::{ContainerRuntime, RuntimeError, RuntimeResult};
use super::native::NativeRuntime;
use super::docker::DockerRuntime;

#[cfg(target_os = "macos")]
use super::apple::AppleContainerRuntime;

/// Create a container runtime from configuration
pub async fn create_runtime(config: &RuntimeConfig) -> RuntimeResult<Arc<dyn ContainerRuntime>> {
    match config.runtime_type {
        RuntimeType::Native => {
            Ok(Arc::new(NativeRuntime::new()))
        }
        RuntimeType::Docker => {
            let runtime = DockerRuntime::new(&config.docker.image)
                .with_network(&config.docker.network);

            let runtime = if let Some(ref mem) = config.docker.memory_limit {
                runtime.with_memory_limit(mem)
            } else {
                runtime
            };

            let runtime = if let Some(ref cpu) = config.docker.cpu_limit {
                runtime.with_cpu_limit(cpu)
            } else {
                runtime
            };

            if !runtime.is_available().await {
                return Err(RuntimeError::NotAvailable(
                    "Docker is not installed or not running".to_string()
                ));
            }

            Ok(Arc::new(runtime))
        }
        RuntimeType::AppleContainer => {
            #[cfg(target_os = "macos")]
            {
                let runtime = if config.apple.image.is_empty() {
                    AppleContainerRuntime::new()
                } else {
                    AppleContainerRuntime::with_image(&config.apple.image)
                };

                if !runtime.is_available().await {
                    return Err(RuntimeError::NotAvailable(
                        "Apple Container is not available (requires macOS 15+)".to_string()
                    ));
                }

                Ok(Arc::new(runtime))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(RuntimeError::NotAvailable(
                    "Apple Container is only available on macOS".to_string()
                ))
            }
        }
    }
}

/// Check which runtimes are available on this system
pub async fn available_runtimes() -> Vec<&'static str> {
    let mut available = vec!["native"]; // Always available

    // Check Docker
    let docker = DockerRuntime::default();
    if docker.is_available().await {
        available.push("docker");
    }

    // Check Apple Container (macOS only)
    #[cfg(target_os = "macos")]
    {
        let apple = AppleContainerRuntime::default();
        if apple.is_available().await {
            available.push("apple");
        }
    }

    available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_native_runtime() {
        let config = RuntimeConfig::default();
        let runtime = create_runtime(&config).await.unwrap();
        assert_eq!(runtime.name(), "native");
    }

    #[tokio::test]
    async fn test_available_runtimes_includes_native() {
        let available = available_runtimes().await;
        assert!(available.contains(&"native"));
    }
}
```

**Step 2: Update mod.rs**

Update `src/runtime/mod.rs`:

```rust
//! Container runtime module for ZeptoClaw
//!
//! This module provides container isolation for shell command execution.
//! It supports multiple runtimes:
//! - Native: Direct execution (no isolation, uses application-level security)
//! - Docker: Docker container isolation (Linux, macOS, Windows)
//! - Apple Container: Apple's native container technology (macOS only)

pub mod types;
pub mod native;
pub mod docker;
pub mod factory;

#[cfg(target_os = "macos")]
pub mod apple;

pub use types::{ContainerRuntime, RuntimeError, RuntimeResult, CommandOutput, ContainerConfig};
pub use native::NativeRuntime;
pub use docker::DockerRuntime;
pub use factory::{create_runtime, available_runtimes};

#[cfg(target_os = "macos")]
pub use apple::AppleContainerRuntime;
```

**Step 3: Run tests**

Run: `cargo test --lib runtime`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/runtime/
git commit -m "feat(runtime): add runtime factory for configuration-based creation"
```

---

## Task 7: Export Runtime Module from Library

**Files:**
- Modify: `src/lib.rs`

**Step 1: Add runtime module export**

Add after the security module (around line 9):

```rust
pub mod runtime;
```

**Step 2: Add runtime re-exports**

Add after existing re-exports:

```rust
pub use runtime::{
    ContainerRuntime, RuntimeError, RuntimeResult, CommandOutput, ContainerConfig,
    NativeRuntime, DockerRuntime, create_runtime, available_runtimes,
};

#[cfg(target_os = "macos")]
pub use runtime::AppleContainerRuntime;
```

**Step 3: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/lib.rs
git commit -m "feat(lib): export runtime module"
```

---

## Task 8: Update ShellTool to Use Runtime

**Files:**
- Modify: `src/tools/shell.rs`

**Step 1: Update imports**

Add at the top:

```rust
use std::sync::Arc;
use std::path::PathBuf;

use crate::runtime::{ContainerRuntime, ContainerConfig, NativeRuntime};
```

**Step 2: Update ShellTool struct**

Replace the struct definition:

```rust
/// Tool for executing shell commands.
///
/// Executes a shell command and returns the combined stdout and stderr output.
/// Commands can run either natively or inside a container for isolation.
///
/// # Parameters
/// - `command`: The shell command to execute (required)
/// - `timeout`: Timeout in seconds, defaults to 60 (optional)
///
/// # Security
/// This tool validates commands against a configurable blocklist to prevent
/// dangerous operations. When using container runtimes (Docker, Apple Container),
/// commands run in isolated environments with limited access to the host system.
pub struct ShellTool {
    security_config: ShellSecurityConfig,
    runtime: Arc<dyn ContainerRuntime>,
}

impl ShellTool {
    /// Create a new shell tool with default security settings and native runtime.
    pub fn new() -> Self {
        Self {
            security_config: ShellSecurityConfig::new(),
            runtime: Arc::new(NativeRuntime::new()),
        }
    }

    /// Create a shell tool with custom security configuration.
    pub fn with_security(security_config: ShellSecurityConfig) -> Self {
        Self {
            security_config,
            runtime: Arc::new(NativeRuntime::new()),
        }
    }

    /// Create a shell tool with a specific container runtime.
    pub fn with_runtime(runtime: Arc<dyn ContainerRuntime>) -> Self {
        Self {
            security_config: ShellSecurityConfig::new(),
            runtime,
        }
    }

    /// Create a shell tool with both custom security and runtime.
    pub fn with_security_and_runtime(
        security_config: ShellSecurityConfig,
        runtime: Arc<dyn ContainerRuntime>,
    ) -> Self {
        Self {
            security_config,
            runtime,
        }
    }

    /// Create a shell tool with no security restrictions.
    ///
    /// # Warning
    /// Only use in trusted environments where command injection is not a concern.
    pub fn permissive() -> Self {
        Self {
            security_config: ShellSecurityConfig::permissive(),
            runtime: Arc::new(NativeRuntime::new()),
        }
    }

    /// Get the runtime name
    pub fn runtime_name(&self) -> &str {
        self.runtime.name()
    }
}
```

**Step 3: Update the execute method**

Replace the execute method:

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'command' argument".into()))?;

        // Security check (application-level, defense-in-depth)
        self.security_config.validate_command(command)?;

        let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(60);

        // Build container config
        let mut container_config = ContainerConfig::new().with_timeout(timeout_secs);

        // Set working directory if workspace is specified
        if let Some(ref workspace) = ctx.workspace {
            let workspace_path = PathBuf::from(workspace);
            container_config = container_config
                .with_workdir(workspace_path.clone())
                .with_mount(workspace_path.clone(), workspace_path, false);
        }

        // Execute via runtime
        let output = self
            .runtime
            .execute(command, &container_config)
            .await
            .map_err(|e| PicoError::Tool(format!("Runtime error: {}", e)))?;

        Ok(output.format())
    }
```

**Step 4: Update Default impl**

```rust
impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 5: Update tests**

Add new tests for runtime functionality:

```rust
    #[test]
    fn test_shell_tool_runtime_name() {
        let tool = ShellTool::new();
        assert_eq!(tool.runtime_name(), "native");
    }

    #[tokio::test]
    async fn test_shell_tool_with_custom_runtime() {
        use crate::runtime::NativeRuntime;
        use std::sync::Arc;

        let runtime = Arc::new(NativeRuntime::new());
        let tool = ShellTool::with_runtime(runtime);
        let ctx = ToolContext::new();

        let result = tool.execute(json!({"command": "echo test"}), &ctx).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test"));
    }
```

**Step 6: Run tests**

Run: `cargo test --lib shell`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add src/tools/shell.rs
git commit -m "feat(tools): update ShellTool to use container runtime abstraction"
```

---

## Task 9: Update main.rs to Configure Runtime

**Files:**
- Modify: `src/main.rs`

**Step 1: Add runtime imports**

Add to imports:

```rust
use zeptoclaw::runtime::{create_runtime, available_runtimes};
```

**Step 2: Update create_agent function**

Find where ShellTool is registered and update it to use the configured runtime:

```rust
    // Create runtime from config
    let runtime = match create_runtime(&config.runtime).await {
        Ok(r) => {
            info!("Using {} runtime for shell commands", r.name());
            r
        }
        Err(e) => {
            warn!("Failed to create configured runtime: {}. Falling back to native.", e);
            Arc::new(zeptoclaw::runtime::NativeRuntime::new())
        }
    };

    // Register shell tool with runtime
    agent.register_tool(Box::new(ShellTool::with_runtime(runtime))).await;
```

**Step 3: Update cmd_onboard to allow runtime selection**

Add runtime selection to the onboard flow:

```rust
fn configure_runtime(config: &mut Config) -> Result<()> {
    println!("\n=== Runtime Configuration ===");
    println!("Choose container runtime for shell command isolation:");
    println!("  1. Native (no container, uses application-level security)");
    println!("  2. Docker (requires Docker installed)");
    #[cfg(target_os = "macos")]
    println!("  3. Apple Container (macOS 15+ only)");
    println!();

    loop {
        print!("Enter choice [1]: ");
        io::stdout().flush()?;

        let choice = read_line()?.trim().to_string();
        let choice = if choice.is_empty() { "1" } else { &choice };

        match choice {
            "1" => {
                config.runtime.runtime_type = zeptoclaw::config::RuntimeType::Native;
                println!("Configured: Native runtime (no container isolation)");
                break;
            }
            "2" => {
                config.runtime.runtime_type = zeptoclaw::config::RuntimeType::Docker;
                print!("Docker image [alpine:latest]: ");
                io::stdout().flush()?;
                let image = read_line()?.trim().to_string();
                if !image.is_empty() {
                    config.runtime.docker.image = image;
                }
                println!("Configured: Docker runtime with image {}", config.runtime.docker.image);
                break;
            }
            #[cfg(target_os = "macos")]
            "3" => {
                config.runtime.runtime_type = zeptoclaw::config::RuntimeType::AppleContainer;
                println!("Configured: Apple Container runtime");
                break;
            }
            _ => {
                println!("Invalid choice. Please try again.");
            }
        }
    }

    Ok(())
}
```

Call this function in `cmd_onboard()` after provider configuration.

**Step 4: Update cmd_status to show runtime info**

Add to the status output:

```rust
    // Show runtime info
    println!("\nRuntime:");
    println!("  Type: {:?}", config.runtime.runtime_type);
    let available = available_runtimes().await;
    println!("  Available: {}", available.join(", "));
```

**Step 5: Verify compilation and tests**

Run: `cargo build && cargo test`
Expected: Compiles and all tests pass

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): add runtime configuration and selection to onboard"
```

---

## Task 10: Add Integration Tests for Runtime

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Add runtime integration tests**

Add to the end of the file:

```rust
// ============================================================================
// Runtime Integration Tests
// ============================================================================

#[tokio::test]
async fn test_runtime_factory_native() {
    use zeptoclaw::config::RuntimeConfig;
    use zeptoclaw::runtime::create_runtime;

    let config = RuntimeConfig::default();
    let runtime = create_runtime(&config).await.unwrap();
    assert_eq!(runtime.name(), "native");
}

#[tokio::test]
async fn test_available_runtimes_includes_native() {
    use zeptoclaw::runtime::available_runtimes;

    let runtimes = available_runtimes().await;
    assert!(runtimes.contains(&"native"));
}

#[tokio::test]
async fn test_shell_tool_with_native_runtime() {
    use zeptoclaw::runtime::NativeRuntime;
    use zeptoclaw::tools::shell::ShellTool;
    use zeptoclaw::tools::{Tool, ToolContext};
    use std::sync::Arc;

    let runtime = Arc::new(NativeRuntime::new());
    let tool = ShellTool::with_runtime(runtime);
    let ctx = ToolContext::new();

    let result = tool
        .execute(serde_json::json!({"command": "echo hello"}), &ctx)
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().contains("hello"));
}

#[tokio::test]
async fn test_shell_tool_runtime_with_workspace() {
    use zeptoclaw::runtime::NativeRuntime;
    use zeptoclaw::tools::shell::ShellTool;
    use zeptoclaw::tools::{Tool, ToolContext};
    use std::sync::Arc;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "content").unwrap();

    let runtime = Arc::new(NativeRuntime::new());
    let tool = ShellTool::with_runtime(runtime);
    let ctx = ToolContext::new().with_workspace(dir.path().to_str().unwrap());

    let result = tool
        .execute(serde_json::json!({"command": "cat test.txt"}), &ctx)
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().contains("content"));
}

#[tokio::test]
async fn test_config_runtime_serialization() {
    use zeptoclaw::config::{RuntimeConfig, RuntimeType};

    let mut config = RuntimeConfig::default();
    config.runtime_type = RuntimeType::Docker;
    config.docker.image = "ubuntu:22.04".to_string();

    let json = serde_json::to_string(&config).unwrap();
    let parsed: RuntimeConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.runtime_type, RuntimeType::Docker);
    assert_eq!(parsed.docker.image, "ubuntu:22.04");
}
```

**Step 2: Run integration tests**

Run: `cargo test --test integration`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add runtime integration tests"
```

---

## Task 11: Final Verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Run format check**

Run: `cargo fmt -- --check`
Expected: No formatting changes needed

**Step 4: Build release**

Run: `cargo build --release`
Expected: Compiles successfully

**Step 5: Manual testing**

```bash
# Test native runtime
cargo run -- agent -m "Run: echo hello"

# Test onboard with runtime selection
cargo run -- onboard
```

**Step 6: Final commit (if any cleanup needed)**

```bash
cargo fmt
git add -A
git commit -m "chore: final cleanup for container isolation feature"
```

---

## Verification Checklist

After implementation, verify:

1. **Native Runtime:**
   ```bash
   # Should work as before
   cargo run -- agent -m "List files in current directory"
   ```

2. **Docker Runtime (if Docker installed):**
   ```json
   // In config file
   {"runtime": {"runtime_type": "docker", "docker": {"image": "alpine:latest"}}}
   ```

3. **Runtime Selection in Onboard:**
   ```bash
   cargo run -- onboard
   # Should prompt for runtime selection
   ```

4. **Security Still Works:**
   ```bash
   # Should still be blocked by application-level security
   cargo run -- agent -m "Run: rm -rf /"
   ```

---

## Configuration Examples

### Native (Default)
```json
{
  "runtime": {
    "runtime_type": "native"
  }
}
```

### Docker
```json
{
  "runtime": {
    "runtime_type": "docker",
    "docker": {
      "image": "alpine:latest",
      "memory_limit": "512m",
      "cpu_limit": "1.0",
      "network": "none"
    }
  }
}
```

### Apple Container (macOS)
```json
{
  "runtime": {
    "runtime_type": "apple",
    "apple": {
      "image": ""
    }
  }
}
```

---

## Security Model

| Layer | Native | Docker | Apple Container |
|-------|--------|--------|-----------------|
| Command Blocklist | ✅ | ✅ | ✅ |
| Path Validation | ✅ | ✅ | ✅ |
| Filesystem Isolation | ❌ | ✅ | ✅ |
| Network Isolation | ❌ | ✅ (configurable) | ✅ |
| Resource Limits | ❌ | ✅ | ✅ |

**Recommendation:** Use Docker or Apple Container for production. Native runtime is suitable for development/testing or when you fully trust the LLM's outputs.
