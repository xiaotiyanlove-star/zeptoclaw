//! OpenClaw → ZeptoClaw migration module.
//!
//! Handles detection of OpenClaw installations, config conversion,
//! and skill directory copying.

pub mod config;
pub mod skills;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Summary of what was migrated, skipped, or is not portable.
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// Detected OpenClaw installation directory.
    pub openclaw_dir: PathBuf,
    /// Config fields that were successfully migrated.
    pub config_migrated: Vec<String>,
    /// Config fields that were skipped (field, reason).
    pub config_skipped: Vec<(String, String)>,
    /// Skill names that were copied.
    pub skills_copied: Vec<String>,
    /// Skills that were skipped (name, reason).
    pub skills_skipped: Vec<(String, String)>,
    /// General warnings.
    pub warnings: Vec<String>,
    /// OpenClaw features that have no ZeptoClaw equivalent.
    pub not_portable: Vec<String>,
}

impl MigrationReport {
    pub fn new(openclaw_dir: PathBuf) -> Self {
        Self {
            openclaw_dir,
            ..Default::default()
        }
    }

    /// Print a human-readable summary to stdout.
    pub fn print_summary(&self) {
        println!();
        println!("Migration Report");
        println!("================");
        println!();

        if !self.config_migrated.is_empty() {
            println!("  Config fields migrated: {}", self.config_migrated.len());
            for field in &self.config_migrated {
                println!("    + {}", field);
            }
        }

        if !self.config_skipped.is_empty() {
            println!();
            println!("  Config fields skipped: {}", self.config_skipped.len());
            for (field, reason) in &self.config_skipped {
                println!("    - {} ({})", field, reason);
            }
        }

        if !self.skills_copied.is_empty() {
            println!();
            println!("  Skills copied: {}", self.skills_copied.len());
            for name in &self.skills_copied {
                println!("    + {}", name);
            }
        }

        if !self.skills_skipped.is_empty() {
            println!();
            println!("  Skills skipped: {}", self.skills_skipped.len());
            for (name, reason) in &self.skills_skipped {
                println!("    - {} ({})", name, reason);
            }
        }

        if !self.not_portable.is_empty() {
            println!();
            println!("  Not portable ({}):", self.not_portable.len());
            for item in &self.not_portable {
                println!("    ! {}", item);
            }
        }

        if !self.warnings.is_empty() {
            println!();
            println!("  Warnings:");
            for w in &self.warnings {
                println!("    * {}", w);
            }
        }

        println!();
    }
}

/// Well-known OpenClaw config file names.
const OPENCLAW_CONFIG_NAMES: &[&str] = &[
    "openclaw.json",
    "openclaw.json5",
    "config.json",
    "config.json5",
];

/// Detect an OpenClaw installation directory.
///
/// Checks the following locations in order:
/// 1. `$OPENCLAW_STATE_DIR`
/// 2. `~/.openclaw`
/// 3. `~/.clawdbot`
/// 4. `~/.moldbot`
///
/// Returns `Some(path)` if a directory containing a recognised config file is found.
pub fn detect_openclaw_dir() -> Option<PathBuf> {
    // Check environment variable first.
    if let Ok(dir) = std::env::var("OPENCLAW_STATE_DIR") {
        let p = PathBuf::from(&dir);
        if has_openclaw_config(&p) {
            return Some(p);
        }
    }

    let home = dirs::home_dir()?;
    for name in &[".openclaw", ".clawdbot", ".moldbot"] {
        let candidate = home.join(name);
        if has_openclaw_config(&candidate) {
            return Some(candidate);
        }
    }

    None
}

/// Check whether a directory contains a recognised OpenClaw config file.
fn has_openclaw_config(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    OPENCLAW_CONFIG_NAMES
        .iter()
        .any(|name| dir.join(name).is_file())
}

/// Load and parse an OpenClaw config file as a `serde_json::Value`.
///
/// Uses the `json5` parser so that comments, trailing commas, and unquoted
/// keys are accepted.
pub fn load_openclaw_config(openclaw_dir: &Path) -> Result<serde_json::Value> {
    for name in OPENCLAW_CONFIG_NAMES {
        let path = openclaw_dir.join(name);
        if path.is_file() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let value: serde_json::Value = json5::from_str(&content)
                .with_context(|| format!("Failed to parse {} as JSON5", path.display()))?;
            return Ok(value);
        }
    }

    anyhow::bail!(
        "No OpenClaw config file found in {}. Expected one of: {}",
        openclaw_dir.display(),
        OPENCLAW_CONFIG_NAMES.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_returns_none_when_nothing_exists() {
        // With no home-dir overrides and no env var, detection on a clean system
        // should return None (or Some if the tester happens to have ~/.openclaw).
        // We just verify it doesn't panic.
        let _ = detect_openclaw_dir();
    }

    #[test]
    fn test_has_openclaw_config() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!has_openclaw_config(tmp.path()));

        fs::write(tmp.path().join("openclaw.json5"), "{}").unwrap();
        assert!(has_openclaw_config(tmp.path()));
    }

    #[test]
    fn test_load_openclaw_config_json5() {
        let tmp = tempfile::tempdir().unwrap();
        let content = r#"{
            // This is a JSON5 comment
            models: {
                providers: {
                    anthropic: {
                        apiKey: "sk-ant-test",
                    },
                },
            },
        }"#;
        fs::write(tmp.path().join("openclaw.json5"), content).unwrap();

        let val = load_openclaw_config(tmp.path()).unwrap();
        assert_eq!(
            val["models"]["providers"]["anthropic"]["apiKey"],
            "sk-ant-test"
        );
    }

    #[test]
    fn test_load_openclaw_config_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_openclaw_config(tmp.path());
        assert!(result.is_err());
    }
}
