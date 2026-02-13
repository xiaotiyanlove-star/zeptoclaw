//! Health check and usage metrics module
//!
//! Provides:
//! - `/healthz` liveness endpoint (always 200 if process is running)
//! - `/readyz` readiness endpoint (200 when agent is processing messages)
//! - Periodic usage counter emission (every 60s)
//! - Graceful shutdown usage summary
//!
//! Uses raw TCP + manual HTTP to avoid adding a web framework dependency.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{info, warn};

/// Default health check port
const DEFAULT_HEALTH_PORT: u16 = 9090;

/// Interval between periodic usage flushes (seconds)
const USAGE_FLUSH_INTERVAL_SECS: u64 = 60;

/// Tracks usage counters for the running gateway instance.
///
/// All counters are lock-free atomics for minimal overhead.
#[derive(Debug)]
pub struct UsageMetrics {
    /// Total requests processed
    pub requests: AtomicU64,
    /// Total tool calls executed
    pub tool_calls: AtomicU64,
    /// Total input tokens consumed (from LLM responses)
    pub input_tokens: AtomicU64,
    /// Total output tokens produced (from LLM responses)
    pub output_tokens: AtomicU64,
    /// Total errors encountered
    pub errors: AtomicU64,
    /// Whether the gateway is ready to accept requests
    pub ready: AtomicBool,
}

impl UsageMetrics {
    pub fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            tool_calls: AtomicU64::new(0),
            input_tokens: AtomicU64::new(0),
            output_tokens: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            ready: AtomicBool::new(false),
        }
    }

    /// Record a completed request
    pub fn record_request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record tool call(s)
    pub fn record_tool_calls(&self, count: u64) {
        self.tool_calls.fetch_add(count, Ordering::Relaxed);
    }

    /// Record token usage from an LLM response
    pub fn record_tokens(&self, input: u64, output: u64) {
        self.input_tokens.fetch_add(input, Ordering::Relaxed);
        self.output_tokens.fetch_add(output, Ordering::Relaxed);
    }

    /// Record an error
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark gateway as ready
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }

    /// Emit current usage as a structured JSON log line
    pub fn emit_usage(&self, reason: &str) {
        info!(
            event = "usage_summary",
            reason = reason,
            requests = self.requests.load(Ordering::Relaxed),
            tool_calls = self.tool_calls.load(Ordering::Relaxed),
            input_tokens = self.input_tokens.load(Ordering::Relaxed),
            output_tokens = self.output_tokens.load(Ordering::Relaxed),
            errors = self.errors.load(Ordering::Relaxed),
            "Usage metrics"
        );
    }
}

impl Default for UsageMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the health check HTTP server.
///
/// Serves:
/// - `GET /healthz` → 200 OK (liveness)
/// - `GET /readyz`  → 200 OK if ready, 503 if not (readiness)
/// - anything else  → 404
///
/// Returns the JoinHandle so the caller can abort on shutdown.
pub async fn start_health_server(
    port: u16,
    metrics: Arc<UsageMetrics>,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!(port = port, "Health server listening");

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    let metrics = Arc::clone(&metrics);
                    tokio::spawn(async move {
                        // Read the request with a 5s timeout to prevent slowloris DoS
                        let mut buf = [0u8; 512];
                        let n = match tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            tokio::io::AsyncReadExt::read(&mut stream, &mut buf),
                        )
                        .await
                        {
                            Ok(Ok(n)) => n,
                            _ => return, // timeout or read error
                        };
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let request_line = request.lines().next().unwrap_or_default();
                        let mut parts = request_line.split_whitespace();
                        let method = parts.next().unwrap_or_default();
                        let raw_path = parts.next().unwrap_or_default();
                        let path = raw_path.split('?').next().unwrap_or(raw_path);

                        let (status, body) = match (method, path) {
                            ("GET", "/healthz") => ("200 OK", "{\"status\":\"ok\"}"),
                            ("GET", "/readyz") => {
                                if metrics.ready.load(Ordering::SeqCst) {
                                    ("200 OK", "{\"status\":\"ready\"}")
                                } else {
                                    ("503 Service Unavailable", "{\"status\":\"not_ready\"}")
                                }
                            }
                            _ => ("404 Not Found", "{\"error\":\"not_found\"}"),
                        };

                        let response = format!(
                            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status,
                            body.len(),
                            body
                        );

                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Health server accept error");
                }
            }
        }
    });

    Ok(handle)
}

/// Start periodic usage metric emission.
///
/// Emits a usage_summary log line every `USAGE_FLUSH_INTERVAL_SECS` seconds.
/// This ensures metrics are captured even if the container is killed (OOM/SIGKILL)
/// before graceful shutdown.
pub fn start_periodic_usage_flush(
    metrics: Arc<UsageMetrics>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(USAGE_FLUSH_INTERVAL_SECS));
        interval.tick().await; // skip first immediate tick

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    metrics.emit_usage("periodic");
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        metrics.emit_usage("shutdown");
                        break;
                    }
                }
            }
        }
    })
}

/// Get the health check port from environment or config, falling back to default.
pub fn health_port() -> u16 {
    std::env::var("ZEPTOCLAW_HEALTH_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_HEALTH_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_metrics_creation() {
        let metrics = UsageMetrics::new();
        assert_eq!(metrics.requests.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.tool_calls.load(Ordering::Relaxed), 0);
        assert!(!metrics.ready.load(Ordering::SeqCst));
    }

    #[test]
    fn test_usage_metrics_recording() {
        let metrics = UsageMetrics::new();
        metrics.record_request();
        metrics.record_request();
        metrics.record_tool_calls(3);
        metrics.record_tokens(100, 50);
        metrics.record_error();

        assert_eq!(metrics.requests.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.tool_calls.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.input_tokens.load(Ordering::Relaxed), 100);
        assert_eq!(metrics.output_tokens.load(Ordering::Relaxed), 50);
        assert_eq!(metrics.errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_ready_flag() {
        let metrics = UsageMetrics::new();
        assert!(!metrics.ready.load(Ordering::SeqCst));
        metrics.set_ready(true);
        assert!(metrics.ready.load(Ordering::SeqCst));
        metrics.set_ready(false);
        assert!(!metrics.ready.load(Ordering::SeqCst));
    }

    #[test]
    fn test_health_port_default() {
        // When env var is not set, should return default
        std::env::remove_var("ZEPTOCLAW_HEALTH_PORT");
        assert_eq!(health_port(), DEFAULT_HEALTH_PORT);
    }

    #[tokio::test]
    async fn test_health_server_responds() {
        let metrics = Arc::new(UsageMetrics::new());
        metrics.set_ready(true);

        // Use port 0 for random available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let handle = start_health_server(port, Arc::clone(&metrics))
            .await
            .unwrap();

        // Give server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Test /healthz
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("200 OK"));
        assert!(response.contains("\"status\":\"ok\""));

        // Test prefix path does not match /healthz
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /healthz-extra HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("404"));
        assert!(response.contains("\"error\":\"not_found\""));

        // Test /readyz when ready
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /readyz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("200 OK"));
        assert!(response.contains("\"status\":\"ready\""));

        // Test /readyz when not ready
        metrics.set_ready(false);
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /readyz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("503"));
        assert!(response.contains("\"status\":\"not_ready\""));

        // Test POST /healthz returns 404 (only GET allowed)
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"POST /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("404"));

        // Test /healthz?foo=bar (query string stripped, should match)
        metrics.set_ready(true);
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /healthz?foo=bar HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("200 OK"));
        assert!(response.contains("\"status\":\"ok\""));

        handle.abort();
    }
}
