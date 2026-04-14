use anyhow::{bail, Result};
use crate::parser::schema::{InlineLogSpec, InstallerManifest, PageType, InstallStep};

/// Validates a parsed manifest for logical consistency.
/// Returns Ok(()) or a descriptive error.
pub fn validate(manifest: &InstallerManifest) -> Result<()> {
    validate_dsl_mode(manifest)?;
    validate_app(manifest)?;
    validate_variables(manifest)?;
    validate_pages(manifest)?;
    validate_install_dsl(manifest)?;
    validate_components(manifest)?;
    validate_steps(manifest)?;
    Ok(())
}

fn validate_dsl_mode(manifest: &InstallerManifest) -> Result<()> {
    if manifest
        .legacy_steps
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        bail!(
            "HG-YAML-001: legacy 'steps' format is no longer supported. Use the top-level 'install' DSL block"
        );
    }
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

fn validate_variables(manifest: &InstallerManifest) -> Result<()> {
    let Some(vars) = &manifest.variables else { return Ok(()); };

    let reserved = [
        "INSTDIR",
        "PROGRAMFILES",
        "PROGRAMFILES64",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "WINDIR",
    ];

    for key in vars.keys() {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            bail!("HG-YAML-001: variables keys must not be empty");
        }

        let normalized = trimmed.trim_start_matches('$');
        if normalized.is_empty() {
            bail!("HG-YAML-001: variables key '{}' is invalid", key);
        }
        if reserved.contains(&normalized) {
            bail!(
                "HG-YAML-001: variables key '{}' attempts to override reserved variable '${}'",
                key,
                normalized
            );
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            bail!(
                "HG-YAML-001: variables key '{}' is invalid. Use only A-Z, 0-9, and '_' (optionally prefixed with '$')",
                key
            );
        }
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

    for install_component_id in manifest.install.components.keys() {
        if !ids.contains(install_component_id.as_str()) {
            bail!(
                "HG-YAML-001: install.components entry '{}' has no matching component in top-level components",
                install_component_id
            );
        }
    }

    Ok(())
}

fn validate_install_dsl(manifest: &InstallerManifest) -> Result<()> {
    if manifest.install.setup.create_dirs.is_empty() {
        bail!("HG-YAML-001: install.setup.create_dirs must contain at least one path");
    }

    if manifest.install.components.is_empty() {
        bail!("HG-YAML-001: install.components must contain at least one component entry");
    }

    for (component_id, spec) in &manifest.install.components {
        if spec.archive.trim().is_empty() {
            bail!(
                "HG-YAML-001: install.components.{}.archive must be non-empty",
                component_id
            );
        }
        if spec.target.trim().is_empty() {
            bail!(
                "HG-YAML-001: install.components.{}.target must be non-empty",
                component_id
            );
        }
    }

    if manifest.install.system.register_uninstall.is_none() {
        bail!("HG-YAML-001: install.system.register_uninstall block is required");
    }

    if let Some(register_uninstall) = &manifest.install.system.register_uninstall {
        if register_uninstall.key.trim().is_empty() {
            bail!("HG-YAML-001: install.system.register_uninstall.key must be non-empty");
        }
    }

    if let Some(path) = &manifest.install.system.path {
        if path.add.trim().is_empty() {
            bail!("HG-YAML-001: install.system.path.add must be non-empty");
        }
    }

    if manifest.install.finalize.write_uninstaller.trim().is_empty() {
        bail!("HG-YAML-001: install.finalize.write_uninstaller must be non-empty");
    }

    if let Some(hooks) = &manifest.install.hooks {
        if let Some(post_install) = &hooks.post_install {
            for (idx, hook) in post_install.iter().enumerate() {
                if hook.run.command.trim().is_empty() {
                    bail!(
                        "HG-YAML-001: install.hooks.post_install[{}].run.command must be non-empty",
                        idx
                    );
                }
            }
        }
    }

    Ok(())
}

fn validate_steps(manifest: &InstallerManifest) -> Result<()> {
    if let Some(logging) = &manifest.logging {
        if let Some(threshold) = logging.slow_step_warn_sec {
            if threshold == 0 {
                bail!("HG-YAML-001: logging.slow_step_warn_sec must be greater than 0");
            }
        }
    }

    let has_file_logging_step = manifest
        .steps
        .iter()
        .any(|s| {
            inline_log_spec(s)
                    .map(|log| log.file.is_some() || log.both.is_some())
                    .unwrap_or(false)
        });
    if has_file_logging_step {
        let Some(logging) = &manifest.logging else {
            bail!("HG-YAML-001: 'logging' block is required when using inline log.file or log.both");
        };
        if logging.path.as_deref().unwrap_or(" ").trim().is_empty() {
            bail!("HG-YAML-001: logging.path must be set when using inline log.file or log.both");
        }
        if logging.file_name.as_deref().unwrap_or(" ").trim().is_empty() {
            bail!("HG-YAML-001: logging.file_name must be set when using inline log.file or log.both");
        }
    }

    for (i, step) in manifest.steps.iter().enumerate() {
        if let Some(spec) = inline_log_spec(step) {
            validate_inline_log(i, spec)?;
        }

        match step {
            InstallStep::Registry(r) => {
                let valid_hives = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC"];
                if !valid_hives.contains(&r.hive.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid registry hive '{}'. Must be one of: {:?}", i, r.hive, valid_hives);
                }
            }
            InstallStep::RegisterUninstall(r) => {
                let valid_hives = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC"];
                if !valid_hives.contains(&r.hive.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid register_uninstall hive '{}'. Must be one of: {:?}", i, r.hive, valid_hives);
                }
                if r.key.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty key", i);
                }
                if r.display_name.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty display_name/name", i);
                }
                if r.display_version.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty display_version/version", i);
                }
                if r.publisher.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty publisher", i);
                }
                if r.install_location.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty install_location/inst_loc", i);
                }
                if r.uninstall_string.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_uninstall' requires non-empty uninstall_string/uninstall", i);
                }
            }
            InstallStep::RegisterApp(r) => {
                let valid_hives = ["HKLM", "HKCU", "HKCR", "HKU", "HKCC"];
                if !valid_hives.contains(&r.hive.as_str()) {
                    bail!("HG-YAML-001: Step {} invalid register_app hive '{}'. Must be one of: {:?}", i, r.hive, valid_hives);
                }
                if r.key.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_app' requires non-empty key", i);
                }
                if r.install_location.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_app' requires non-empty install_location/inst_loc", i);
                }
                if r.version.trim().is_empty() {
                    bail!("HG-YAML-001: Step {} action 'register_app' requires non-empty version", i);
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

fn inline_log_spec(step: &InstallStep) -> Option<&InlineLogSpec> {
    match step {
        InstallStep::Extract(s) => s.log.as_ref(),
        InstallStep::CopyFile(s) => s.log.as_ref(),
        InstallStep::DeleteFile(s) => s.log.as_ref(),
        InstallStep::CreateDir(s) => s.log.as_ref(),
        InstallStep::Registry(s) => s.log.as_ref(),
        InstallStep::RegisterUninstall(s) => s.log.as_ref(),
        InstallStep::RegisterApp(s) => s.log.as_ref(),
        InstallStep::Shortcut(s) => s.log.as_ref(),
        InstallStep::EnvVar(s) => s.log.as_ref(),
        InstallStep::Service(s) => s.log.as_ref(),
        InstallStep::RunProgram(s) => s.log.as_ref(),
        InstallStep::RunPowerShell(s) => s.log.as_ref(),
        InstallStep::WriteUninstaller(s) => s.log.as_ref(),
    }
}

fn validate_inline_log(step_index: usize, spec: &InlineLogSpec) -> Result<()> {
    let present = [spec.both.as_ref(), spec.ui.as_ref(), spec.file.as_ref()]
        .into_iter()
        .flatten()
        .count();

    if present != 1 {
        bail!(
            "HG-YAML-001: Step {} inline 'log' must contain exactly one of: both, ui, file",
            step_index
        );
    }

    if let Some(msg) = spec.both.as_ref().or(spec.ui.as_ref()).or(spec.file.as_ref()) {
        if msg.trim().is_empty() {
            bail!(
                "HG-YAML-001: Step {} inline 'log' message must be non-empty",
                step_index
            );
        }
    }

    Ok(())
}