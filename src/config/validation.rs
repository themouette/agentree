use super::Config;
use std::path::Path;

/// A non-fatal configuration warning
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigWarning {
    pub message: String,
    pub file: Option<String>,
}

impl ConfigWarning {
    /// Create a new warning without file context
    pub fn new(message: String) -> Self {
        Self {
            message,
            file: None,
        }
    }

    /// Create a new warning with file context
    pub fn with_file(message: String, file: String) -> Self {
        Self {
            message,
            file: Some(file),
        }
    }
}

/// Validate configuration and return warnings and errors
///
/// Returns (warnings, errors) where:
/// - Warnings are non-fatal issues (e.g., path doesn't exist yet, missing {branch} in template)
/// - Errors are fatal issues that prevent using the config
///
/// Note: Backend validation happens during deserialization (serde) and CLI override
/// validation happens in Config::with_cli_overrides(). This function focuses on
/// semantic checks that can't be caught by the type system.
pub fn validate(config: &Config) -> (Vec<ConfigWarning>, Vec<String>) {
    let mut warnings = Vec::new();
    let errors = Vec::new();

    // Check if worktree location exists (warning, not error - will be created when needed)
    if let Some(location) = &config.worktree.location {
        let path = Path::new(location);
        if !path.exists() {
            warnings.push(ConfigWarning::new(format!(
                "Worktree location '{}' does not exist. It will be created when needed.",
                location
            )));
        }
    }

    // Validate template
    let template_warnings = validate_template(&config.worktree.template);
    warnings.extend(template_warnings);

    (warnings, errors)
}

/// Validate worktree template and return warnings
pub fn validate_template(template: &str) -> Vec<ConfigWarning> {
    let mut warnings = Vec::new();

    // Check template is not empty
    if template.is_empty() {
        warnings.push(ConfigWarning::new(
            "Template is empty. This may cause issues when creating worktrees.".to_string(),
        ));
    }

    // Check template doesn't start with / (absolute path)
    if template.starts_with('/') {
        warnings.push(ConfigWarning::new(format!(
            "Template '{}' starts with '/'. Absolute paths in templates may be unsafe.",
            template
        )));
    }

    // Check template doesn't contain .. (path traversal)
    if template.contains("..") {
        warnings.push(ConfigWarning::new(format!(
            "Template '{}' contains '..' path traversal. This may be unsafe.",
            template
        )));
    }

    // Check template contains {branch} (warning - may cause collisions)
    if !template.contains("{branch}") {
        warnings.push(ConfigWarning::new(format!(
            "Template '{}' does not contain {{branch}}. Different branches may resolve to the same worktree path.",
            template
        )));
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config_no_warnings() {
        let config = Config::default();
        let (warnings, errors) = validate(&config);

        assert!(warnings.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_nonexistent_location_produces_warning() {
        let mut config = Config::default();
        config.worktree.location = Some("/tmp/nonexistent-path-12345".to_string());

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("does not exist"));
        assert!(warnings[0].message.contains("/tmp/nonexistent-path-12345"));
    }

    #[test]
    fn test_template_without_branch_produces_warning() {
        let mut config = Config::default();
        config.worktree.template = "{repo}".to_string();

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("does not contain {branch}"));
        assert!(warnings[0]
            .message
            .contains("may resolve to the same worktree path"));
    }

    #[test]
    fn test_template_with_dotdot_produces_warning() {
        let mut config = Config::default();
        config.worktree.template = "../{branch}".to_string();

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        // Should have 2 warnings: .. traversal + missing {branch} (since it has {branch})
        // Wait, it has {branch}, so just 1 warning for ..
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains(".."));
        assert!(warnings[0].message.contains("may be unsafe"));
    }

    #[test]
    fn test_template_starting_with_slash_produces_warning() {
        let mut config = Config::default();
        config.worktree.template = "/{branch}".to_string();

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("starts with '/'"));
        assert!(warnings[0].message.contains("may be unsafe"));
    }

    #[test]
    fn test_empty_template_produces_warning() {
        let mut config = Config::default();
        config.worktree.template = "".to_string();

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        // Empty template produces 2 warnings: empty + missing {branch}
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("Template is empty")));
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("does not contain {branch}")));
    }

    #[test]
    fn test_default_config_produces_no_warnings() {
        let config = Config::default();
        let (warnings, errors) = validate(&config);

        assert!(warnings.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_template_empty() {
        let warnings = validate_template("");

        assert_eq!(warnings.len(), 2); // empty + missing {branch}
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("Template is empty")));
    }

    #[test]
    fn test_validate_template_with_dotdot() {
        let warnings = validate_template("../{branch}");

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains(".."));
    }

    #[test]
    fn test_validate_template_absolute_path() {
        let warnings = validate_template("/tmp/{branch}");

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("starts with '/'"));
    }

    #[test]
    fn test_validate_template_missing_branch() {
        let warnings = validate_template("{repo}");

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("does not contain {branch}"));
    }

    #[test]
    fn test_validate_template_valid() {
        let warnings = validate_template("{repo}/{branch}");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_config_warning_creation() {
        let w1 = ConfigWarning::new("Test message".to_string());
        assert_eq!(w1.message, "Test message");
        assert_eq!(w1.file, None);

        let w2 = ConfigWarning::with_file("Test message".to_string(), "test.toml".to_string());
        assert_eq!(w2.message, "Test message");
        assert_eq!(w2.file, Some("test.toml".to_string()));
    }

    #[test]
    fn test_multiple_template_issues() {
        // Template with both .. and missing {branch}
        let mut config = Config::default();
        config.worktree.template = "../repo".to_string();

        let (warnings, errors) = validate(&config);

        assert_eq!(errors.len(), 0);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().any(|w| w.message.contains("..")));
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("does not contain {branch}")));
    }
}
