use crate::error::Result;
use std::path::Path;

use super::Agent;

/// No-op agent implementation for unknown agent names.
///
/// `prepare` returns immediately with `Ok(())`, `cleanup` does nothing.
/// Used automatically for agents without a dedicated implementation
/// (custom/unknown tools).
pub struct GenericAgent;

impl GenericAgent {
    pub fn new() -> Self {
        GenericAgent
    }
}

impl Default for GenericAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for GenericAgent {
    type PrepareToken = ();

    fn prepare(&self, _workspace_path: &Path) -> Result<Self::PrepareToken> {
        Ok(())
    }

    fn cleanup(&self, _workspace_path: &Path, _token: &Self::PrepareToken) {
        // no-op
    }

    fn name(&self) -> &str {
        "generic"
    }
}
