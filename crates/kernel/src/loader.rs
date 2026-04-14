use ai_os_shared::contract::ContractManifest;
use ai_os_shared::error::{AiOsError, Result};
use std::path::Path;

/// Load a compiled contract manifest from a JSON file.
pub fn load_manifest(path: &Path) -> Result<ContractManifest> {
    let content = std::fs::read_to_string(path).map_err(AiOsError::Io)?;
    let manifest: ContractManifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_error() {
        let result = load_manifest(Path::new("/nonexistent/contract.json"));
        assert!(result.is_err());
    }
}
