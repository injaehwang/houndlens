use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::HookAction;

const PRE_COMMIT_HOOK: &str = r#"#!/bin/sh
# omnilens pre-commit hook — semantic verification before commit
# Installed by: omnilens hook install

# Get staged files
STAGED=$(git diff --cached --name-only --diff-filter=ACM)
if [ -z "$STAGED" ]; then
    exit 0
fi

echo "[omnilens] Verifying staged changes..."

# Run omnilens verify on staged changes
omnilens verify --format text 2>&1
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo ""
    echo "[omnilens] ❌ Verification failed. Fix issues above or commit with --no-verify to bypass."
    exit 1
fi

echo "[omnilens] ✅ Verification passed."
exit 0
"#;

const PRE_PUSH_HOOK: &str = r#"#!/bin/sh
# omnilens pre-push hook — full verification before push
# Installed by: omnilens hook install

REMOTE="$1"
URL="$2"

echo "[omnilens] Verifying changes before push..."

# Find the merge base with the remote branch
LOCAL_SHA=$(git rev-parse HEAD)
REMOTE_SHA=$(git rev-parse @{u} 2>/dev/null || echo "HEAD~10")

omnilens --format json verify --diff "$REMOTE_SHA" > /tmp/omnilens-push-result.json 2>/dev/null
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    BREAKING=$(cat /tmp/omnilens-push-result.json 2>/dev/null | grep -o '"breaking":[0-9]*' | grep -o '[0-9]*')
    if [ "${BREAKING:-0}" -gt 0 ]; then
        echo "[omnilens] ❌ Push blocked: $BREAKING breaking changes detected."
        echo "[omnilens] Run 'omnilens verify --diff $REMOTE_SHA' for details."
        exit 1
    fi
fi

echo "[omnilens] ✅ Push verification passed."
exit 0
"#;

pub fn run(action: HookAction) -> Result<()> {
    let hooks_dir = find_git_hooks_dir()?;

    match action {
        HookAction::Install => install(&hooks_dir),
        HookAction::Uninstall => uninstall(&hooks_dir),
        HookAction::Status => status(&hooks_dir),
    }
}

fn install(hooks_dir: &PathBuf) -> Result<()> {
    fs::create_dir_all(hooks_dir)?;

    let pre_commit = hooks_dir.join("pre-commit");
    let pre_push = hooks_dir.join("pre-push");

    // Backup existing hooks.
    if pre_commit.exists() {
        let backup = hooks_dir.join("pre-commit.backup");
        if !backup.exists() {
            fs::copy(&pre_commit, &backup)?;
            println!(
                "  {} existing pre-commit → pre-commit.backup",
                "Backed up".yellow()
            );
        }
    }
    if pre_push.exists() {
        let backup = hooks_dir.join("pre-push.backup");
        if !backup.exists() {
            fs::copy(&pre_push, &backup)?;
            println!(
                "  {} existing pre-push → pre-push.backup",
                "Backed up".yellow()
            );
        }
    }

    fs::write(&pre_commit, PRE_COMMIT_HOOK)?;
    fs::write(&pre_push, PRE_PUSH_HOOK)?;

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&pre_push, fs::Permissions::from_mode(0o755))?;
    }

    println!("{} Git hooks installed:", "Done.".green().bold());
    println!("  {} pre-commit → semantic verification before commit", "✓".green());
    println!("  {} pre-push   → breaking change detection before push", "✓".green());
    println!(
        "\n  {} Use {} to bypass hooks when needed.",
        "Tip:".dimmed(),
        "--no-verify".cyan()
    );

    Ok(())
}

fn uninstall(hooks_dir: &PathBuf) -> Result<()> {
    let pre_commit = hooks_dir.join("pre-commit");
    let pre_push = hooks_dir.join("pre-push");

    let mut removed = 0;

    if pre_commit.exists() && is_omnilens_hook(&pre_commit)? {
        fs::remove_file(&pre_commit)?;
        // Restore backup if exists.
        let backup = hooks_dir.join("pre-commit.backup");
        if backup.exists() {
            fs::rename(&backup, &pre_commit)?;
            println!("  {} pre-commit (restored backup)", "Removed".yellow());
        } else {
            println!("  {} pre-commit", "Removed".yellow());
        }
        removed += 1;
    }

    if pre_push.exists() && is_omnilens_hook(&pre_push)? {
        fs::remove_file(&pre_push)?;
        let backup = hooks_dir.join("pre-push.backup");
        if backup.exists() {
            fs::rename(&backup, &pre_push)?;
            println!("  {} pre-push (restored backup)", "Removed".yellow());
        } else {
            println!("  {} pre-push", "Removed".yellow());
        }
        removed += 1;
    }

    if removed == 0 {
        println!("{} No omnilens hooks found.", "Info:".dimmed());
    } else {
        println!("{} {} hook(s) removed.", "Done.".green().bold(), removed);
    }

    Ok(())
}

fn status(hooks_dir: &PathBuf) -> Result<()> {
    println!("{} {}", "Hooks dir:".bold(), hooks_dir.display());

    for name in &["pre-commit", "pre-push"] {
        let path = hooks_dir.join(name);
        if path.exists() {
            if is_omnilens_hook(&path)? {
                println!("  {} {} (omnilens)", "✓".green(), name);
            } else {
                println!("  {} {} (other — not omnilens)", "●".yellow(), name);
            }
        } else {
            println!("  {} {} (not installed)", "✗".dimmed(), name);
        }
    }

    Ok(())
}

fn find_git_hooks_dir() -> Result<PathBuf> {
    // Check for custom hooks path (git config core.hooksPath).
    let output = std::process::Command::new("git")
        .args(["config", "core.hooksPath"])
        .output()
        .ok();

    if let Some(out) = output {
        if out.status.success() {
            let custom = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !custom.is_empty() {
                return Ok(PathBuf::from(custom));
            }
        }
    }

    // Default: .git/hooks
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .context("Not a git repository")?;

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(git_dir).join("hooks"))
}

fn is_omnilens_hook(path: &PathBuf) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    Ok(content.contains("omnilens"))
}
