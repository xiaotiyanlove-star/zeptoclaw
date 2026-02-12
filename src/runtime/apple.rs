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
        let output = tokio::time::timeout(Duration::from_secs(config.timeout_secs), cmd.output())
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

    #[test]
    fn test_apple_runtime_default() {
        let runtime = AppleContainerRuntime::default();
        assert!(runtime.image.is_none());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "requires Apple Container framework"]
    async fn test_apple_runtime_available() {
        let runtime = AppleContainerRuntime::new();
        // This checks if the container tool is available
        println!(
            "Apple Container available: {}",
            runtime.is_available().await
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn test_apple_runtime_not_available_on_non_macos() {
        let runtime = AppleContainerRuntime::new();
        assert!(!runtime.is_available().await);
    }
}
