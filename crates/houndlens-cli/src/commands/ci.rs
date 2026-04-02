use std::process::Command;

use anyhow::Result;
use colored::Colorize;

use crate::OutputFormat;

/// Detected CI platform.
enum Platform {
    GitHub,
    GitLab,
    Local,
}

pub fn run(platform_override: Option<&str>, fail_on: &str, format: &OutputFormat) -> Result<()> {
    let platform = if let Some(p) = platform_override {
        match p {
            "github" => Platform::GitHub,
            "gitlab" => Platform::GitLab,
            _ => Platform::Local,
        }
    } else {
        detect_platform()
    };

    let platform_name = match &platform {
        Platform::GitHub => "GitHub Actions",
        Platform::GitLab => "GitLab CI",
        Platform::Local => "Local",
    };

    eprintln!(
        "{} {} detected",
        "Platform:".bold(),
        platform_name.cyan()
    );

    // Determine diff range based on platform.
    let diff_ref = get_diff_ref(&platform);
    eprintln!(
        "{} {}",
        "Diff base:".bold(),
        diff_ref.as_deref().unwrap_or("(working directory)")
    );

    // Build engine and run verification.
    let mut engine = super::create_engine()?;
    let idx = engine.index()?;
    eprintln!(
        "{} {} files, {} links",
        "Indexed:".bold(),
        idx.files_analyzed,
        idx.links_resolved
    );

    let diff_spec = if let Some(ref base) = diff_ref {
        houndlens_core::verify::DiffSpec::GitDiff {
            base: base.clone(),
            head: "HEAD".to_string(),
        }
    } else {
        houndlens_core::verify::DiffSpec::WorkingDir
    };

    let result = engine.verify(&diff_spec)?;

    // Output based on format.
    match format {
        OutputFormat::Json => {
            println!("{}", houndlens_core::output::to_json(&result));
        }
        OutputFormat::Sarif => {
            println!("{}", houndlens_core::output::to_sarif(&result));
        }
        OutputFormat::Text => {
            print_text_summary(&result);
        }
    }

    // Platform-specific post-processing.
    match &platform {
        Platform::GitHub => write_github_output(&result),
        Platform::GitLab => write_gitlab_output(&result),
        Platform::Local => {}
    }

    // Determine exit code.
    let should_fail = match fail_on {
        "never" => false,
        "warning" => result.warning_count() > 0 || result.has_errors(),
        _ => result.has_errors(), // "error" default
    };

    if should_fail {
        std::process::exit(1);
    }

    Ok(())
}

fn detect_platform() -> Platform {
    if std::env::var("GITHUB_ACTIONS").is_ok() {
        Platform::GitHub
    } else if std::env::var("GITLAB_CI").is_ok() {
        Platform::GitLab
    } else {
        Platform::Local
    }
}

fn get_diff_ref(platform: &Platform) -> Option<String> {
    match platform {
        Platform::GitHub => {
            // PR: GITHUB_BASE_REF, Push: compare with previous commit.
            std::env::var("GITHUB_BASE_REF")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| format!("origin/{}", s))
                .or_else(|| Some("HEAD~1".to_string()))
        }
        Platform::GitLab => {
            // MR: CI_MERGE_REQUEST_DIFF_BASE_SHA, Pipeline: CI_COMMIT_BEFORE_SHA.
            std::env::var("CI_MERGE_REQUEST_DIFF_BASE_SHA")
                .ok()
                .filter(|s| !s.is_empty() && s != "0000000000000000000000000000000000000000")
                .or_else(|| {
                    std::env::var("CI_COMMIT_BEFORE_SHA")
                        .ok()
                        .filter(|s| !s.is_empty() && s != "0000000000000000000000000000000000000000")
                })
                .or_else(|| Some("HEAD~1".to_string()))
        }
        Platform::Local => {
            // Local: check if there are uncommitted changes.
            let has_changes = Command::new("git")
                .args(["diff", "--name-only"])
                .output()
                .ok()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false);

            let has_staged = Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .output()
                .ok()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false);

            if has_changes || has_staged {
                Some("HEAD".to_string())
            } else {
                Some("HEAD~1".to_string())
            }
        }
    }
}

fn print_text_summary(result: &houndlens_core::verify::VerifyResult) {
    let status = if result.has_errors() {
        "FAIL".red().bold()
    } else {
        "PASS".green().bold()
    };

    println!(
        "\n{} | {} changes | {} warnings | Risk: {:.0}%",
        status,
        result.semantic_changes.len(),
        result.warning_count(),
        result.risk_score * 100.0,
    );

    for c in &result.semantic_changes {
        let badge = match c.risk {
            houndlens_core::verify::ChangeRisk::Breaking => "BREAKING".red(),
            houndlens_core::verify::ChangeRisk::SecuritySensitive => "SECURITY".red(),
            houndlens_core::verify::ChangeRisk::NeedsReview => "REVIEW".yellow(),
            houndlens_core::verify::ChangeRisk::Safe => "SAFE".green(),
        };
        let file = c.location.file.file_name().unwrap_or_default().to_string_lossy();
        println!("  [{}] {}:{} — {}", badge, file, c.location.start_line, c.description);
    }
}

fn write_github_output(result: &houndlens_core::verify::VerifyResult) {
    // Write to $GITHUB_OUTPUT if available.
    if let Ok(output_file) = std::env::var("GITHUB_OUTPUT") {
        let content = format!(
            "status={}\nrisk-score={:.0}\nchanges={}\n",
            if result.has_errors() { "fail" } else { "pass" },
            result.risk_score * 100.0,
            result.semantic_changes.len(),
        );
        let _ = std::fs::OpenOptions::new()
            .append(true)
            .open(output_file)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(content.as_bytes())
            });
    }

    // Write step summary if available.
    if let Ok(summary_file) = std::env::var("GITHUB_STEP_SUMMARY") {
        let md = format_markdown_summary(result);
        let _ = std::fs::OpenOptions::new()
            .append(true)
            .open(summary_file)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(md.as_bytes())
            });
    }
}

fn write_gitlab_output(result: &houndlens_core::verify::VerifyResult) {
    // GitLab: write dotenv artifact for downstream jobs.
    let content = format!(
        "HOUNDLENS_STATUS={}\nHOUNDLENS_RISK={:.0}\nHOUNDLENS_CHANGES={}\n",
        if result.has_errors() { "fail" } else { "pass" },
        result.risk_score * 100.0,
        result.semantic_changes.len(),
    );
    let _ = std::fs::write("houndlens.env", content);
}

fn format_markdown_summary(result: &houndlens_core::verify::VerifyResult) -> String {
    let status = if result.has_errors() { "❌ FAIL" } else { "✅ PASS" };
    let mut md = format!(
        "## {} houndlens verification\n\n| Metric | Value |\n|--------|-------|\n",
        status
    );
    md.push_str(&format!(
        "| Risk Score | **{:.0}%** |\n| Changes | {} |\n| Breaking | {} |\n| Warnings | {} |\n\n",
        result.risk_score * 100.0,
        result.semantic_changes.len(),
        result.semantic_changes.iter().filter(|c| matches!(c.risk, houndlens_core::verify::ChangeRisk::Breaking)).count(),
        result.warning_count(),
    ));
    md
}
