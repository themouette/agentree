use crate::{
    error::{AgentreeError, Result},
    version,
};
use clap::Args;
use self_update::cargo_crate_version;

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Version to update to (e.g., v1.2.3, 1.2.3, or 'latest')
    pub version: Option<String>,

    /// Check for updates without installing
    #[arg(long)]
    pub check: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Execute the update command
pub fn execute(args: UpdateArgs) -> Result<()> {
    if args.check {
        check_and_display()
    } else {
        perform_update(args.version, args.yes)
    }
}

/// Check for updates and display information without installing
fn check_and_display() -> Result<()> {
    let current = version::VERSION;
    println!("Current version: {}", current);
    println!("\nChecking for updates...");

    match get_latest_version()? {
        Some(latest) if latest != current => {
            println!("New version available: {}", latest);
            println!("\nRun 'agentree update' to upgrade");
        }
        Some(_) => println!("You're already running the latest version"),
        None => println!("Unable to check for updates"),
    }
    Ok(())
}

/// Perform the actual update to latest or specified version
fn perform_update(target: Option<String>, skip_confirm: bool) -> Result<()> {
    let current = version::VERSION;
    println!("Current version: {}", current);

    // Handle version format: strip 'v' prefix if present
    let target_version = match target {
        Some(v) if v == "latest" => None,
        Some(v) => Some(v.trim_start_matches('v').to_string()),
        None => None,
    };

    // Fetch latest if not specified
    let target_version = if target_version.is_none() {
        match get_latest_version()? {
            Some(latest) => {
                if latest == current {
                    println!("You're already running the latest version");
                    return Ok(());
                }
                println!("New version available: {}", latest);
                Some(latest)
            }
            None => {
                return Err(AgentreeError::UpdateError(
                    "Unable to fetch latest version".into(),
                ))
            }
        }
    } else {
        target_version
    };

    println!("\nDownloading update...");

    let mut update_builder = self_update::backends::github::Update::configure();
    update_builder
        .repo_owner(version::REPO_OWNER)
        .repo_name(version::REPO_NAME)
        .bin_name(version::binary_name())
        .target(&version::current_platform()?)
        .current_version(cargo_crate_version!())
        .show_download_progress(true)
        .no_confirm(skip_confirm);

    if let Some(version) = target_version {
        update_builder.target_version_tag(&format!("v{}", version));
    }

    let update = update_builder
        .build()
        .map_err(|e| AgentreeError::UpdateError(e.to_string()))?;

    let status = match update.update() {
        Ok(status) => status,
        Err(e) => {
            let err_string = e.to_string();
            if err_string.contains("Permission denied") || err_string.contains("EACCES") {
                return Err(AgentreeError::PermissionDenied(
                    "Cannot replace binary. Try: sudo agentree update".into(),
                ));
            }
            return Err(AgentreeError::UpdateError(e.to_string()));
        }
    };

    println!("\nSuccessfully updated to version {}", status.version());
    Ok(())
}

/// Get the latest version from GitHub releases
///
/// Returns None on network/API errors for graceful degradation
pub fn get_latest_version() -> Result<Option<String>> {
    match self_update::backends::github::ReleaseList::configure()
        .repo_owner(version::REPO_OWNER)
        .repo_name(version::REPO_NAME)
        .build()
    {
        Ok(releases) => match releases.fetch() {
            Ok(releases) => {
                if let Some(release) = releases.first() {
                    let version = release.version.trim_start_matches('v').to_string();
                    Ok(Some(version))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_args_parsing() {
        // Just verify the struct compiles and has expected fields
        let args = UpdateArgs {
            version: Some("1.2.3".to_string()),
            check: false,
            yes: true,
        };
        assert_eq!(args.version, Some("1.2.3".to_string()));
        assert!(!args.check);
        assert!(args.yes);
    }

    #[test]
    fn test_version_strip() {
        // Test that we handle v-prefix correctly
        let with_v = Some("v1.2.3".to_string());
        let stripped = with_v.map(|v| v.trim_start_matches('v').to_string());
        assert_eq!(stripped, Some("1.2.3".to_string()));

        let without_v = Some("1.2.3".to_string());
        let stripped = without_v.map(|v| v.trim_start_matches('v').to_string());
        assert_eq!(stripped, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_latest_keyword() {
        let latest = Some("latest".to_string());
        let result = match latest {
            Some(v) if v == "latest" => None,
            other => other,
        };
        assert_eq!(result, None);
    }
}
