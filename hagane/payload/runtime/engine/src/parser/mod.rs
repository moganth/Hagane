pub mod schema;
pub mod validator;

use anyhow::Result;
use schema::InstallerManifest;

/// Load and validate a manifest from a YAML string (embedded or on-disk).
pub fn load_from_str(yaml: &str) -> Result<InstallerManifest> {
    let manifest: InstallerManifest = serde_yaml::from_str(yaml)
        .map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;
    validator::validate(&manifest)?;
    Ok(manifest)
}

/// Load and validate a manifest from a file path.
pub fn load_from_file(path: &std::path::Path) -> Result<InstallerManifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read manifest '{}': {}", path.display(), e))?;
    load_from_str(&content)
}