//! Runtime factory for creating container runtimes from configuration

use std::sync::Arc;

use crate::config::{RuntimeConfig, RuntimeType};

use super::docker::DockerRuntime;
use super::native::NativeRuntime;
use super::types::{ContainerRuntime, RuntimeError, RuntimeResult};

#[cfg(target_os = "macos")]
use super::apple::AppleContainerRuntime;

/// Create a container runtime from configuration
pub async fn create_runtime(config: &RuntimeConfig) -> RuntimeResult<Arc<dyn ContainerRuntime>> {
    match config.runtime_type {
        RuntimeType::Native => Ok(Arc::new(NativeRuntime::new())),
        RuntimeType::Docker => {
            let runtime =
                DockerRuntime::new(&config.docker.image).with_network(&config.docker.network);

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
                    "Docker is not installed or not running".to_string(),
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
                        "Apple Container is not available (requires macOS 15+)".to_string(),
                    ));
                }

                Ok(Arc::new(runtime))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(RuntimeError::NotAvailable(
                    "Apple Container is only available on macOS".to_string(),
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
