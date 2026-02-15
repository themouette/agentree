use crate::error::{AgentreeError, Result};
use semver::Version;

/// Version string injected at build time from Cargo.toml and git info
pub const VERSION: &str = env!("AGENTREE_VERSION");

/// Package name from Cargo.toml
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// GitHub repository owner
pub const REPO_OWNER: &str = "themouette";

/// GitHub repository name
pub const REPO_NAME: &str = "agentree";

/// Detect current platform in format matching release artifacts
///
/// Returns platform string like "macos-aarch64" or "linux-x86_64" that matches
/// the naming convention used in GitHub release artifacts and install.sh script.
///
/// # Errors
///
/// Returns UpdateError if the current OS/architecture combination is not supported
pub fn current_platform() -> Result<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("macos-aarch64".to_string()),
        ("macos", "x86_64") => Ok("macos-x86_64".to_string()),
        ("linux", "aarch64") => Ok("linux-aarch64".to_string()),
        ("linux", "x86_64") => Ok("linux-x86_64".to_string()),
        (os, arch) => Err(AgentreeError::UpdateError(format!(
            "Unsupported platform: {}-{}",
            os, arch
        ))),
    }
}

/// Get the binary name for this application
pub fn binary_name() -> &'static str {
    PKG_NAME
}

/// Check if the given version is newer than the current version
///
/// Uses semantic versioning comparison. Returns false if either version
/// cannot be parsed as valid semver.
pub fn is_newer_version(other: &str) -> bool {
    match (Version::parse(VERSION), Version::parse(other)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant_exists() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.contains('.'));
    }

    #[test]
    fn test_pkg_name_constant() {
        assert_eq!(PKG_NAME, "agentree");
    }

    #[test]
    fn test_repo_constants() {
        assert_eq!(REPO_OWNER, "themouette");
        assert_eq!(REPO_NAME, "agentree");
    }

    #[test]
    fn test_current_platform() {
        let platform = current_platform().unwrap();
        // Should match one of the supported platforms
        assert!(
            platform == "macos-aarch64"
                || platform == "macos-x86_64"
                || platform == "linux-aarch64"
                || platform == "linux-x86_64"
        );
    }

    #[test]
    fn test_binary_name() {
        assert_eq!(binary_name(), "agentree");
    }

    #[test]
    fn test_is_newer_version() {
        // Valid semver comparisons
        assert!(is_newer_version("999.0.0"));
        assert!(!is_newer_version("0.0.1"));

        // Invalid semver should return false
        assert!(!is_newer_version("invalid"));
        assert!(!is_newer_version(""));
    }
}
