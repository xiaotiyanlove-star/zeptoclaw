//! OpenClaw skill directory detection and copying.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

/// Result of copying skills: (copied_names, skipped_with_reasons).
pub type CopySkillsResult = (Vec<String>, Vec<(String, String)>);

/// Find OpenClaw skill directories.
///
/// Checks:
/// - `<openclaw_dir>/skills/`
/// - Workspace directory from config, if set (e.g. `~/projects/skills/`)
pub fn find_skill_dirs(openclaw_dir: &Path, openclaw_config: &Value) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Primary: <openclaw_dir>/skills/
    let primary = openclaw_dir.join("skills");
    if primary.is_dir() {
        dirs.push(primary);
    }

    // From config workspace
    if let Some(workspace) = openclaw_config
        .get("agents")
        .and_then(|a| a.get("defaults"))
        .and_then(|d| d.get("workspace"))
        .and_then(|w| w.as_str())
    {
        let ws_path = expand_tilde(workspace);
        let ws_skills = ws_path.join("skills");
        if ws_skills.is_dir() && !dirs.contains(&ws_skills) {
            dirs.push(ws_skills);
        }
    }

    dirs
}

/// Copy skills from source directories to the destination directory.
///
/// Each skill is a subdirectory containing a `SKILL.md` file. Skills that
/// already exist in the destination are skipped.
///
/// Returns `(copied_names, skipped)` where skipped contains `(name, reason)`.
pub fn copy_skills(source_dirs: &[PathBuf], dest_dir: &Path) -> Result<CopySkillsResult> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create skills directory: {}", dest_dir.display()))?;

    let mut copied = Vec::new();
    let mut skipped = Vec::new();

    for source_dir in source_dirs {
        let entries = match std::fs::read_dir(source_dir) {
            Ok(e) => e,
            Err(err) => {
                skipped.push((
                    source_dir.display().to_string(),
                    format!("Failed to read directory: {}", err),
                ));
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Only copy directories that contain SKILL.md
            if !path.join("SKILL.md").is_file() {
                continue;
            }

            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let dest_skill = dest_dir.join(&name);
            if dest_skill.exists() {
                skipped.push((name, "already exists in destination".into()));
                continue;
            }

            match copy_dir_recursive(&path, &dest_skill) {
                Ok(()) => copied.push(name),
                Err(err) => skipped.push((name, format!("copy failed: {}", err))),
            }
        }
    }

    Ok((copied, skipped))
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Expand `~/` prefix to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_skill_dirs_primary() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let config = serde_json::json!({});
        let dirs = find_skill_dirs(tmp.path(), &config);
        assert_eq!(dirs, vec![skills_dir]);
    }

    #[test]
    fn test_find_skill_dirs_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let config = serde_json::json!({});
        let dirs = find_skill_dirs(tmp.path(), &config);
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_copy_skills() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create a valid skill directory
        let skill_dir = src.path().join("skills").join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\n---\nHello",
        )
        .unwrap();
        fs::write(skill_dir.join("helper.sh"), "#!/bin/sh\necho hi").unwrap();

        // Create a non-skill directory (no SKILL.md)
        let non_skill = src.path().join("skills").join("not-a-skill");
        fs::create_dir_all(&non_skill).unwrap();
        fs::write(non_skill.join("README.md"), "readme").unwrap();

        let dest_dir = dst.path().join("skills");
        let (copied, skipped) = copy_skills(&[src.path().join("skills")], &dest_dir).unwrap();

        assert_eq!(copied, vec!["my-skill".to_string()]);
        assert!(skipped.is_empty());

        // Verify files were copied
        assert!(dest_dir.join("my-skill").join("SKILL.md").is_file());
        assert!(dest_dir.join("my-skill").join("helper.sh").is_file());
        // Non-skill should not be copied
        assert!(!dest_dir.join("not-a-skill").exists());
    }

    #[test]
    fn test_copy_skills_skips_existing() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create source skill
        let skill_dir = src.path().join("skills").join("existing");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: existing\n---").unwrap();

        // Pre-create destination skill
        let dest_dir = dst.path().join("skills");
        fs::create_dir_all(dest_dir.join("existing")).unwrap();

        let (copied, skipped) = copy_skills(&[src.path().join("skills")], &dest_dir).unwrap();

        assert!(copied.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].0, "existing");
        assert!(skipped[0].1.contains("already exists"));
    }
}
