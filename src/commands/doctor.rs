use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{doctor, validation};
use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug)]
pub struct DoctorArgs {
    /// Interactively fix detected issues
    #[arg(long)]
    pub fix: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    /// Human-readable format with colored output
    Human,
    /// Machine-readable JSON format
    Json,
}

#[derive(Serialize)]
struct JsonOutput {
    scan_time: String,
    issues: Vec<JsonIssue>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonIssue {
    #[serde(rename = "type")]
    issue_type: String,
    path: String,
    branch: Option<String>,
    description: String,
    fix: String,
}

#[derive(Serialize)]
struct JsonSummary {
    total: usize,
    errors: usize,
    warnings: usize,
}

pub fn execute(args: DoctorArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Load config
    let config = config::load(&repo_root)?;

    // Run diagnostics
    let report = doctor::diagnose(&repo_root, &config.worktree)?;

    // Display results
    match args.format {
        OutputFormat::Human => display_human(&report),
        OutputFormat::Json => {
            display_json(&report)?;
            // JSON mode always exits with success (machine-readable output)
            return Ok(());
        }
    }

    // If --fix flag is set, run interactive fixing
    if args.fix {
        if report.issues.is_empty() {
            // No issues to fix
            return Ok(());
        }

        eprintln!("\nStarting interactive fix mode...");
        eprintln!();

        let fix_report = doctor::fix_issues_interactive(&report)?;

        // Display fix summary
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("Fix Summary:");
        eprintln!("  Fixed:   {}", fix_report.fixed);
        eprintln!("  Skipped: {}", fix_report.skipped);
        eprintln!("  Failed:  {}", fix_report.failed.len());

        if !fix_report.failed.is_empty() {
            eprintln!();
            eprintln!("Failed fixes:");
            for (issue, error) in &fix_report.failed {
                eprintln!("  - {}: {}", issue.path.display(), error);
            }
        }

        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Exit with error if any fixes failed
        if !fix_report.failed.is_empty() {
            return Err(crate::error::AgentreeError::Worktree(format!(
                "{} issue(s) failed to fix",
                fix_report.failed.len()
            )));
        }
    } else if !report.issues.is_empty() {
        // Exit with error code if issues found (for CI usage)
        // This applies to both errors and warnings
        let (errors, warnings) = report.count_by_severity();
        return Err(crate::error::AgentreeError::Worktree(format!(
            "Found {} error(s) and {} warning(s)",
            errors, warnings
        )));
    }

    Ok(())
}

fn display_human(report: &doctor::DiagnosticReport) {
    eprintln!("Scanning worktrees for issues...");
    eprintln!();

    if report.issues.is_empty() {
        println!("✓ No issues found. All worktrees are healthy!");
        return;
    }

    let (errors, warnings) = report.count_by_severity();
    eprintln!("Found {} issue(s):", report.issues.len());
    eprintln!();

    // Display each issue
    for (index, issue) in report.issues.iter().enumerate() {
        doctor::display_issue(issue, index, report.issues.len());
    }

    // Summary
    eprintln!("Summary: {} error(s), {} warning(s)", errors, warnings);
    eprintln!("Run with --fix to interactively fix these issues.");
}

fn display_json(report: &doctor::DiagnosticReport) -> Result<()> {
    let (errors, warnings) = report.count_by_severity();

    let json_issues: Vec<JsonIssue> = report
        .issues
        .iter()
        .map(|issue| {
            let issue_type = match issue.issue_type {
                doctor::IssueType::OrphanedDirectory => "orphaned_directory",
                doctor::IssueType::BrokenMetadata => "broken_metadata",
            };

            JsonIssue {
                issue_type: issue_type.to_string(),
                path: issue.path.display().to_string(),
                branch: issue.branch.clone(),
                description: issue.description.clone(),
                fix: issue.fix_description.clone(),
            }
        })
        .collect();

    let output = JsonOutput {
        scan_time: format!("{:?}", report.scan_time),
        issues: json_issues,
        summary: JsonSummary {
            total: report.issues.len(),
            errors,
            warnings,
        },
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
