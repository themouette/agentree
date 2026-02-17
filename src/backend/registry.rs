use crate::backend::exec::get_binary_version;
use crate::backend::BackendKind;
use crate::error::{AgentreeError, Result};
use std::collections::HashMap;

/// Information about a backend and its requirements
#[derive(Debug, Clone)]
struct BackendInfo {
    binary_name: String,
    min_version: Option<semver::VersionReq>,
    install_instructions: String,
    update_instructions: String,
}

/// Registry for validating and creating backends
#[derive(Debug)]
pub struct BackendRegistry {
    backends: HashMap<BackendKind, BackendInfo>,
}

impl BackendRegistry {
    /// Create a new registry with default backend information
    pub fn new() -> Self {
        let mut backends = HashMap::new();

        // Local backend - no binary requirement
        backends.insert(
            BackendKind::Local,
            BackendInfo {
                binary_name: String::new(),
                min_version: None,
                install_instructions: String::new(),
                update_instructions: String::new(),
            },
        );

        // Claude-vm backend
        backends.insert(
            BackendKind::ClaudeVm,
            BackendInfo {
                binary_name: "claude-vm".to_string(),
                min_version: None, // No minimum version requirement yet
                install_instructions: "brew install claude-vm  # or: cargo install claude-vm"
                    .to_string(),
                update_instructions: "brew upgrade claude-vm  # or: cargo install claude-vm"
                    .to_string(),
            },
        );

        // Docker Sandbox backend
        backends.insert(
            BackendKind::DockerSandbox,
            BackendInfo {
                binary_name: "docker".to_string(),
                min_version: Some(
                    semver::VersionReq::parse(">=29.1.5")
                        .expect("Valid version requirement for docker-sandbox"),
                ),
                install_instructions:
                    "Download Docker Desktop 4.58+ from: https://www.docker.com/products/docker-desktop/"
                        .to_string(),
                update_instructions:
                    "Update Docker Desktop to 4.58+ from: https://www.docker.com/products/docker-desktop/"
                        .to_string(),
            },
        );

        Self { backends }
    }

    /// Validate that a backend is available and meets requirements
    pub fn validate(&self, kind: &BackendKind) -> Result<()> {
        // Local backend is always available
        if matches!(kind, BackendKind::Local) {
            return Ok(());
        }

        // Docker Sandbox has special validation requirements
        if matches!(kind, BackendKind::DockerSandbox) {
            return self.validate_docker_sandbox();
        }

        // Get backend info
        let info = self
            .backends
            .get(kind)
            .ok_or_else(|| AgentreeError::BackendNotFound {
                name: kind.to_string(),
                available: vec![
                    "local".to_string(),
                    "claude-vm".to_string(),
                    "docker-sandbox".to_string(),
                ],
            })?;

        // Check if binary exists in PATH
        if which::which(&info.binary_name).is_err() {
            return Err(AgentreeError::BackendBinaryNotFound {
                backend: kind.to_string(),
                binary: info.binary_name.clone(),
                install_instructions: info.install_instructions.clone(),
            });
        }

        // Check version requirement if specified
        if let Some(ref min_version) = info.min_version {
            let version_str = get_binary_version(&info.binary_name, "--version")?;
            let version =
                semver::Version::parse(&version_str).map_err(|e| AgentreeError::VersionParse {
                    version: version_str.clone(),
                    error: e.to_string(),
                })?;

            if !min_version.matches(&version) {
                return Err(AgentreeError::BackendVersionTooOld {
                    backend: kind.to_string(),
                    current: version.to_string(),
                    minimum: min_version.to_string(),
                    update_instructions: info.update_instructions.clone(),
                });
            }
        }

        Ok(())
    }

    /// Special validation for Docker Sandbox backend
    fn validate_docker_sandbox(&self) -> Result<()> {
        // Check platform - microVMs not supported on Linux
        #[cfg(target_os = "linux")]
        return Err(AgentreeError::DockerSandboxLinuxNotSupported);

        #[cfg(not(target_os = "linux"))]
        {
            // Get backend info
            let info = self
                .backends
                .get(&BackendKind::DockerSandbox)
                .expect("DockerSandbox backend should be registered");

            // Check if docker binary exists in PATH
            if which::which(&info.binary_name).is_err() {
                return Err(AgentreeError::BackendBinaryNotFound {
                    backend: "docker-sandbox".to_string(),
                    binary: info.binary_name.clone(),
                    install_instructions: info.install_instructions.clone(),
                });
            }

            // Check if Docker daemon is running
            let info_check = std::process::Command::new(&info.binary_name)
                .arg("info")
                .output();

            match info_check {
                Ok(output) if !output.status.success() => {
                    return Err(AgentreeError::DockerNotRunning);
                }
                Err(_) => {
                    return Err(AgentreeError::DockerNotRunning);
                }
                _ => {}
            }

            // Check Docker version
            if let Some(ref min_version) = info.min_version {
                let version_str = get_binary_version(&info.binary_name, "--version")?;
                let version =
                    semver::Version::parse(&version_str).map_err(|e| AgentreeError::VersionParse {
                        version: version_str.clone(),
                        error: e.to_string(),
                    })?;

                if !min_version.matches(&version) {
                    return Err(AgentreeError::DockerSandboxNotSupported {
                        current: version.to_string(),
                        minimum_engine: "29.1.5".to_string(),
                        minimum_desktop: "4.58".to_string(),
                    });
                }
            }

            Ok(())
        }
    }

    /// Create a backend after validation
    pub fn create_backend(&self, kind: &BackendKind) -> Result<crate::backend::BackendType> {
        self.validate(kind)?;
        Ok(crate::backend::BackendType::from_kind(*kind))
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Backend;

    #[test]
    fn test_registry_new_has_all_backends() {
        let registry = BackendRegistry::new();
        assert!(registry.backends.contains_key(&BackendKind::Local));
        assert!(registry.backends.contains_key(&BackendKind::ClaudeVm));
        assert!(registry.backends.contains_key(&BackendKind::DockerSandbox));
        assert_eq!(registry.backends.len(), 3);
    }

    #[test]
    fn test_validate_local_always_succeeds() {
        let registry = BackendRegistry::new();
        let result = registry.validate(&BackendKind::Local);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_claude_vm_missing_binary() {
        let registry = BackendRegistry::new();
        // In most test environments, claude-vm won't be installed
        // So this should return BackendBinaryNotFound
        let result = registry.validate(&BackendKind::ClaudeVm);

        // Check if claude-vm is actually installed
        if which::which("claude-vm").is_err() {
            // Expected case: binary not found
            assert!(result.is_err());
            match result {
                Err(AgentreeError::BackendBinaryNotFound {
                    backend,
                    binary,
                    install_instructions,
                }) => {
                    assert_eq!(backend, "claude-vm");
                    assert_eq!(binary, "claude-vm");
                    assert!(
                        install_instructions.contains("brew install")
                            || install_instructions.contains("cargo install")
                    );
                }
                _ => panic!("Expected BackendBinaryNotFound error"),
            }
        } else {
            // Unexpected case: claude-vm is installed in test environment
            // In this case, validation should succeed (no min version set)
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_create_backend_local_succeeds() {
        let registry = BackendRegistry::new();
        let result = registry.create_backend(&BackendKind::Local);
        assert!(result.is_ok());
        let backend = result.unwrap();
        assert_eq!(backend.name(), "local");
    }

    #[test]
    fn test_create_backend_validates_first() {
        let registry = BackendRegistry::new();
        // If claude-vm is not installed, create_backend should fail
        if which::which("claude-vm").is_err() {
            let result = registry.create_backend(&BackendKind::ClaudeVm);
            assert!(result.is_err());
            match result {
                Err(AgentreeError::BackendBinaryNotFound { .. }) => {
                    // Expected
                }
                _ => panic!("Expected BackendBinaryNotFound error"),
            }
        }
    }

    #[test]
    fn test_default_trait() {
        let registry = BackendRegistry::default();
        assert_eq!(registry.backends.len(), 3);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_validate_docker_sandbox_fails_on_linux() {
        let registry = BackendRegistry::new();
        let result = registry.validate(&BackendKind::DockerSandbox);
        assert!(result.is_err());
        match result {
            Err(AgentreeError::DockerSandboxLinuxNotSupported) => {
                // Expected
            }
            _ => panic!("Expected DockerSandboxLinuxNotSupported error"),
        }
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_validate_docker_sandbox_checks_binary() {
        let registry = BackendRegistry::new();
        // This test runs on macOS/Windows
        // If docker is not installed, we should get BackendBinaryNotFound
        if which::which("docker").is_err() {
            let result = registry.validate(&BackendKind::DockerSandbox);
            assert!(result.is_err());
            match result {
                Err(AgentreeError::BackendBinaryNotFound { backend, .. }) => {
                    assert_eq!(backend, "docker-sandbox");
                }
                _ => panic!("Expected BackendBinaryNotFound error"),
            }
        }
    }
}
