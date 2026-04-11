use anyhow::{bail, Result};
use crate::parser::schema::{InstallerManifest, PageType, InstallStep};

/// Validates a parsed manifest for logical consistency.
/// Returns Ok(()) or a descriptive error.
pub fn validate(manifest: &InstallerManifest) -> Result<()> {
    validate_app(manifest)?;
    validate_pages(manifest)?;
    validate_components(manifest)?;
    validate_steps(manifest)?;
    Ok(())
}

fn validate_app(manifest: &InstallerManifest) -> Result<()> {
    let app = &manifest.app;
    if app.name.trim().is_empty() {
        bail!("app.name must not be empty");
    }
    if app.version.trim().is_empty() {
        bail!("app.version must not be empty");
    }
    if app.publisher.trim().is_empty() {
        bail!("app.publisher must not be empty");
    }
    Ok(())
}

fn validate_pages(manifest: &InstallerManifest) -> Result<()> {
    let has_install = manifest.pages.iter().any(|p| p.page_type == PageType::Install);
    if !has_install {
        bail!("pages must include at least one page of type 'install'");
    }
    // If requirements are defined, a requirements page should exist
    if manifest.requirements.is_some() {
        let has_req_page = manifest.pages.iter().any(|p| p.page_type == PageType::Requirements);
        if !has_req_page {
            log::warn!("Requirements are defined but no 'requirements' page is listed — checks will still run silently");
        }
    }
    Ok(())
}

fn validate_components(manifest: &InstallerManifest) -> Result<()> {
    let Some(components) = &manifest.components else { return Ok(()); };
    let ids: std::collections::HashSet<&str> = components.iter().map(|c| c.id.as_str()).collect();
    // Check for duplicate IDs
    if ids.len() != components.len() {
        bail!("Duplicate component IDs detected");
    }
    // Check dependency references exist
    for comp in components {
        if let Some(deps) = &comp.depends_on {
            for dep in deps {
                if !ids.contains(dep.as_str()) {
                    bail!("Component '{}' depends on unknown component '{}'", comp.id, dep);
                }
            }
        }
    }
    Ok(())
}

fn validate_steps(manifest: &InstallerManifest) -> Result<()> {
    for (i, step) in manifest.steps.iter().enumerate() {
        match step {
            InstallStep::Registry(r) => {
                let valid_hives = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC"];
                if !valid_hives.contains(&r.hive.as_str()) {
                    bail!("Step {}: invalid registry hive '{}'. Must be one of: {:?}", i, r.hive, valid_hives);
                }
            }
            InstallStep::EnvVar(e) => {
                let valid_scopes = ["user", "system"];
                if !valid_scopes.contains(&e.scope.as_str()) {
                    bail!("Step {}: invalid env var scope '{}'. Must be 'user' or 'system'", i, e.scope);
                }
                let valid_ops = ["set", "append", "prepend"];
                if !valid_ops.contains(&e.operation.as_str()) {
                    bail!("Step {}: invalid env var operation '{}'. Must be 'set', 'append', or 'prepend'", i, e.operation);
                }
            }
            _ => {}
        }
    }
    Ok(())
}