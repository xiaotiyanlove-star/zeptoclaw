//! `zeptoclaw migrate` — import config and skills from an OpenClaw installation.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};

use zeptoclaw::config::Config;
use zeptoclaw::migrate::{self, MigrationReport};

use super::common::read_line;

/// Run the migration command.
pub(crate) async fn cmd_migrate(from: Option<String>, yes: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("(dry-run mode — no files will be written)");
        println!();
    }

    // ── Step 1: Detect OpenClaw directory ─────────────────────────────
    let openclaw_dir = match from {
        Some(ref path) => {
            let p = PathBuf::from(path);
            if !p.is_dir() {
                anyhow::bail!("Specified path does not exist: {}", p.display());
            }
            p
        }
        None => {
            println!("Searching for OpenClaw installation...");
            match migrate::detect_openclaw_dir() {
                Some(dir) => {
                    println!("  Found: {}", dir.display());
                    dir
                }
                None => {
                    println!("No OpenClaw installation detected.");
                    println!();
                    println!("Checked: ~/.openclaw, ~/.clawdbot, ~/.moldbot, $OPENCLAW_STATE_DIR");
                    println!();
                    println!("Use --from <path> to specify the OpenClaw directory manually.");
                    return Ok(());
                }
            }
        }
    };

    // ── Step 2: Parse config ──────────────────────────────────────────
    println!();
    println!("Loading OpenClaw config from: {}", openclaw_dir.display());

    let openclaw_config = match migrate::load_openclaw_config(&openclaw_dir) {
        Ok(c) => {
            println!("  Config loaded successfully.");
            c
        }
        Err(e) => {
            println!("  Warning: Could not load config: {}", e);
            println!("  Proceeding with skill migration only.");
            serde_json::json!({})
        }
    };

    // Summarise what was found
    let provider_count = openclaw_config
        .get("models")
        .and_then(|m| m.get("providers"))
        .and_then(|p| p.as_object())
        .map(|o| o.len())
        .unwrap_or(0);
    let channel_count = openclaw_config
        .get("channels")
        .and_then(|c| c.as_object())
        .map(|o| o.len())
        .unwrap_or(0);

    let skill_dirs = migrate::skills::find_skill_dirs(&openclaw_dir, &openclaw_config);
    let skill_count: usize = skill_dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|entries| entries.flatten())
        .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").is_file())
        .count();

    println!();
    println!("  Providers found: {}", provider_count);
    println!("  Channels found:  {}", channel_count);
    println!("  Skills found:    {}", skill_count);

    let mut report = MigrationReport::new(openclaw_dir.clone());

    // ── Step 3: Migrate config ────────────────────────────────────────
    let mut config = Config::load().unwrap_or_default();

    println!();
    let do_config = if yes {
        true
    } else {
        print!("Migrate config settings? [Y/n]: ");
        io::stdout().flush()?;
        let answer = read_line()?.to_ascii_lowercase();
        answer.is_empty() || answer == "y" || answer == "yes"
    };

    if do_config {
        let config_result = migrate::config::convert_config(&openclaw_config, &mut config);

        report.config_migrated = config_result.migrated;
        report.config_skipped = config_result.skipped;
        report.not_portable = config_result.not_portable;

        if !dry_run {
            // Back up existing config
            let config_path = Config::path();
            if config_path.exists() {
                let backup_path = config_path.with_extension("json.bak");
                std::fs::copy(&config_path, &backup_path).with_context(|| {
                    format!("Failed to back up config to {}", backup_path.display())
                })?;
                println!("  Backed up existing config to: {}", backup_path.display());
            }

            config
                .save()
                .with_context(|| "Failed to save migrated config")?;
            println!("  Config saved to: {}", Config::path().display());
        } else {
            println!(
                "  (dry-run) Would save config to: {}",
                Config::path().display()
            );
        }
    } else {
        println!("  Skipping config migration.");
    }

    // ── Step 4: Copy skills ───────────────────────────────────────────
    println!();
    let do_skills = if skill_dirs.is_empty() || skill_count == 0 {
        println!("  No skills found to copy.");
        false
    } else if yes {
        true
    } else {
        print!("Copy {} skills to ZeptoClaw? [Y/n]: ", skill_count);
        io::stdout().flush()?;
        let answer = read_line()?.to_ascii_lowercase();
        answer.is_empty() || answer == "y" || answer == "yes"
    };

    if do_skills {
        let dest_dir = Config::dir().join("skills");

        if !dry_run {
            let (copied, skipped) = migrate::skills::copy_skills(&skill_dirs, &dest_dir)?;
            report.skills_copied = copied;
            report.skills_skipped = skipped;
            println!(
                "  Copied {} skills to: {}",
                report.skills_copied.len(),
                dest_dir.display()
            );
        } else {
            println!(
                "  (dry-run) Would copy {} skills to: {}",
                skill_count,
                dest_dir.display()
            );
        }
    }

    // ── Step 5: Validate ──────────────────────────────────────────────
    if do_config && !dry_run {
        println!();
        println!("Validating migrated config...");
        let config_path = Config::path();
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&content) {
                    let diagnostics = zeptoclaw::config::validate::validate_config(&raw);
                    if diagnostics.is_empty() {
                        println!("  Config is valid.");
                    } else {
                        for diag in &diagnostics {
                            println!("  {}", diag);
                        }
                    }
                }
            }
        }
    }

    // ── Step 6: Print report ──────────────────────────────────────────
    report.print_summary();

    if !report.not_portable.is_empty() {
        println!("For details on features that can't be migrated, see:");
        println!("  https://zeptoclaw.pages.dev/docs/guides/migration/");
        println!();
    }

    Ok(())
}
