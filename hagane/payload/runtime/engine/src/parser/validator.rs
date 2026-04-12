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
        bail!("HG-YAML-001: app.name must not be empty");
    }
    if app.version.trim().is_empty() {
        bail!("HG-YAML-001: app.version must not be empty");
    }
    if app.publisher.trim().is_empty() {
        bail!("HG-YAML-001: app.publisher must not be empty");
    }
    Ok(())
}

fn validate_pages(manifest: &InstallerManifest) -> Result<()> {
    let has_install = manifest.pages.iter().any(|p| p.page_type == PageType::Install);
    if !has_install {
        bail!("HG-YAML-001: pages must include at least one page of type 'install'");
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
        bail!("HG-YAML-001: duplicate component IDs detected");
    }
    // Check dependency references exist
    for comp in components {
        if let Some(deps) = &comp.depends_on {
            for dep in deps {
                if !ids.contains(dep.as_str()) {
                    bail!("HG-YAML-001: component '{}' depends on unknown component '{}'", comp.id, dep);
                }
            }
        }
    }
    Ok(())
}

fn validate_steps(manifest: &InstallerManifest) -> Result<()> {
    let has_log_file_step = manifest.steps.iter().any(|s| matches!(s, InstallStep::LogFile(_)));
    if has_log_file_step {
        let Some(logging) = &manifest.logging else {
            bail!("HG-YAML-001: 'logging' block is required when using action 'log_file'");
        };
        if logging.path.as_deref().unwrap_or(" ").trim().is_empty() {
            bail!("HG-YAML-001: logging.path must be set when using action 'log_file'");
        }
        if logging.file_name.as_deref().unwrap_or(" ").trim().is_empty() {
            bail!("HG-YAML-001: logging.file_name must be set when using action 'log_file'");
        }
    }

    for (i, step) in manifest.steps.iter().enumerate() {
        match step {
            InstallStep::LogUi(s) => {
                if s.message.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'log_ui' requires a non-empty message", i);
                }
            }
            InstallStep::LogFile(s) => {
                if s.message.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'log_file' requires a non-empty message", i);
                }
            }
            InstallStep::Registry(r) => {
                let valid_hives = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC"];
                if !valid_hives.contains(&r.hive.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid registry hive '{}'. Must be one of: {:?}", i, r.hive, valid_hives);
                }
            }
            InstallStep::EnvVar(e) => {
                let valid_scopes = ["user", "system"];
                if !valid_scopes.contains(&e.scope.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid env var scope '{}'. Must be 'user' or 'system'", i, e.scope);
                }
                let valid_ops = ["set", "append", "prepend"];
                if !valid_ops.contains(&e.operation.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid env var operation '{}'. Must be 'set', 'append', or 'prepend'", i, e.operation);
                }
                if let Some(component) = &e.component {
                    let Some(components) = &manifest.components else {
                        bail!("HG-YAML-001: Step {} env var component '{}' requires manifest.components to be defined", i, component);
                    };
                    let known = components.iter().any(|c| c.id == *component);
                    if !known {
                        bail!("HG-YAML-001: Step {} env var component '{}' does not match any defined component", i, component);
                    }
                }
            }
            InstallStep::RunPowerShell(s) => {
                let has_script = s.script.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
                let has_file = s.file.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);

                if has_script == has_file {
                    bail!(
                        "HG-YAML-001: Step {} action 'run_powershell' requires exactly one of 'script' or 'file'",
                        i
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}