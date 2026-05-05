pub mod schema;
pub mod validator;

use anyhow::Result;
use schema::{
    CreateDirStep, EnvVarStep, ExtractStep, InstallHookShell, InstallStep, InstallerManifest,
    RegisterAppStep, RegisterUninstallStep, RunPowerShellStep, RunProgramStep, ShortcutStep,
    WriteUninstallerStep,
};

/// Load and validate a manifest from a YAML string (embedded or on-disk).
pub fn load_from_str(yaml: &str) -> Result<InstallerManifest> {
    let mut manifest: InstallerManifest = serde_yaml::from_str(yaml)
        .map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;

    if manifest
        .legacy_steps
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return Err(anyhow::anyhow!(
            "HG-YAML-001: legacy 'steps' format is no longer supported. Use the top-level 'install' DSL block."
        ));
    }

    manifest.steps = compile_install_steps(&manifest)?;
    validator::validate(&manifest)?;
    Ok(manifest)
}

/// Load and validate a manifest from a file path.
pub fn load_from_file(path: &std::path::Path) -> Result<InstallerManifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read manifest '{}': {}", path.display(), e))?;
    load_from_str(&content)
}

fn compile_install_steps(manifest: &InstallerManifest) -> Result<Vec<InstallStep>> {
    let mut steps = Vec::new();
    let install = &manifest.install;

    // Determine current OS for platform-filtered steps.
    let current_os = std::env::consts::OS; // "windows" | "linux" | "macos"

    // ── create_dirs ────────────────────────────────────────────────────────
    for dir in &install.setup.create_dirs {
        steps.push(InstallStep::CreateDir(CreateDirStep {
            path: dir.clone(),
            log: None,
        }));
    }

    // ── component archives ─────────────────────────────────────────────────
    for (component_id, spec) in &install.components {
        steps.push(InstallStep::Extract(ExtractStep {
            archive: spec.archive.clone(),
            destination: spec.target.clone(),
            component: Some(component_id.clone()),
            log: None,
        }));
    }

    // ── system integration — platform-aware ────────────────────────────────
    match current_os {
        "windows" => {
            let sys = install.system.windows_effective();

            if let Some(register_app) = &sys.register_app {
                let key = if let Some(key) = &register_app.key {
                    key.clone()
                } else if let Some(app_key) = &manifest.app.registry_key {
                    format!("SOFTWARE\\{}", app_key)
                } else {
                    return Err(anyhow::anyhow!(
                        "HG-YAML-001: install.system.register_app.key is required when app.registry_key is not set"
                    ));
                };
                steps.push(InstallStep::RegisterApp(RegisterAppStep {
                    hive: register_app.hive.clone().unwrap_or_else(|| "HKLM".to_string()),
                    key,
                    install_location: register_app.install_location.clone().unwrap_or_else(|| "{{INSTDIR}}".to_string()),
                    version: register_app.version.clone().unwrap_or_else(|| manifest.app.version.clone()),
                    log: None,
                }));
            }

            if let Some(reg_uninstall) = &sys.register_uninstall {
                steps.push(InstallStep::RegisterUninstall(RegisterUninstallStep {
                    hive: reg_uninstall.hive.clone().unwrap_or_else(|| "HKLM".to_string()),
                    key: reg_uninstall.key.clone(),
                    display_name: reg_uninstall.name.clone().unwrap_or_else(|| manifest.app.name.clone()),
                    display_version: reg_uninstall.version.clone().unwrap_or_else(|| manifest.app.version.clone()),
                    publisher: reg_uninstall.publisher.clone().unwrap_or_else(|| manifest.app.publisher.clone()),
                    install_location: reg_uninstall.install_location.clone().unwrap_or_else(|| "{{INSTDIR}}".to_string()),
                    uninstall_string: reg_uninstall.uninstall.clone().unwrap_or_else(|| "{{INSTDIR}}/uninstall.exe".to_string()),
                    estimated_size_kb: reg_uninstall.estimated_size_kb,
                    no_modify: reg_uninstall.no_modify.unwrap_or(true),
                    no_repair: reg_uninstall.no_repair.unwrap_or(true),
                    log: None,
                }));
            }

            if let Some(shortcuts) = &sys.shortcuts {
                for shortcut in shortcuts {
                    steps.push(InstallStep::Shortcut(ShortcutStep {
                        target: shortcut.target.clone(),
                        location: shortcut.location.clone(),
                        name: shortcut.name.clone(),
                        description: shortcut.description.clone(),
                        icon: shortcut.icon.clone(),
                        arguments: shortcut.arguments.clone(),
                        working_dir: shortcut.working_dir.clone(),
                        component: shortcut.component.clone(),
                        log: None,
                    }));
                }
            }

            if let Some(path) = &sys.path {
                steps.push(InstallStep::EnvVar(EnvVarStep {
                    name: "Path".to_string(),
                    value: path.add.clone(),
                    scope: path.scope.clone().unwrap_or_else(|| "system".to_string()),
                    operation: "append".to_string(),
                    component: path.component.clone(),
                    log: None,
                }));
            }
        }

        "linux" => {
            if let Some(sys) = install.system.linux_effective() {
                if let Some(cfg) = &sys.config {
                    steps.push(InstallStep::WriteLinuxConfig(schema::WriteLinuxConfigStep {
                        path: cfg.path.clone(),
                        format: cfg.format.clone(),
                        entries: cfg.entries.iter().map(|e| (e.key.clone(), e.value.clone())).collect(),
                        log: None,
                    }));
                }

                if let Some(manifest_dsl) = &sys.uninstall_manifest {
                    steps.push(InstallStep::WriteLinuxConfig(schema::WriteLinuxConfigStep {
                        path: manifest_dsl.path.clone(),
                        format: "json".to_string(),
                        entries: manifest_dsl.entries.iter().map(|e| (e.key.clone(), e.value.clone())).collect(),
                        log: None,
                    }));
                }

                if let Some(de) = &sys.desktop_entry {
                    steps.push(InstallStep::WriteDesktopEntry(schema::WriteDesktopEntryStep {
                        name: de.name.clone(),
                        exec: de.exec.clone(),
                        icon: de.icon.clone(),
                        comment: de.comment.clone(),
                        categories: de.categories.clone(),
                        terminal: de.terminal,
                        location: de.location.clone(),
                        component: de.component.clone(),
                        log: None,
                    }));
                }

                for path in &sys.path {
                    steps.push(InstallStep::EnvVar(EnvVarStep {
                        name: "PATH".to_string(),
                        value: path.add.clone(),
                        scope: path.scope.clone().unwrap_or_else(|| "user".to_string()),
                        operation: "append".to_string(),
                        component: path.component.clone(),
                        log: None,
                    }));
                }
            }
        }

        _ => {}
    }

    // ── post-install hooks — platform-filtered ─────────────────────────────
    if let Some(hooks) = &install.hooks {
        if let Some(post_install) = &hooks.post_install {
            for hook in post_install {
                // Skip if hook specifies a different platform.
                if let Some(p) = &hook.run.platform {
                    if p != current_os { continue; }
                }
                match hook.run.shell {
                    InstallHookShell::Powershell => {
                        steps.push(InstallStep::RunPowerShell(RunPowerShellStep {
                            script: Some(hook.run.command.clone()),
                            file: None,
                            arguments: None,
                            wait: hook.run.wait,
                            fail_on_nonzero: hook.run.fail_on_nonzero,
                            timeout_sec: hook.run.timeout_sec,
                            component: None,
                            log: None,
                        }));
                    }
                    InstallHookShell::Bash => {
                        steps.push(InstallStep::RunBash(schema::RunBashStep {
                            script: Some(hook.run.command.clone()),
                            file: None,
                            wait: hook.run.wait,
                            fail_on_nonzero: hook.run.fail_on_nonzero,
                            timeout_sec: hook.run.timeout_sec,
                            component: None,
                            log: None,
                        }));
                    }
                    InstallHookShell::Program => {
                        steps.push(InstallStep::RunProgram(RunProgramStep {
                            executable: hook.run.command.clone(),
                            arguments: None,
                            wait: hook.run.wait,
                            component: None,
                            log: None,
                        }));
                    }
                }
            }
        }
    }

    // ── finalize: write uninstaller ────────────────────────────────────────
    if let Some(path) = install.finalize.write_uninstaller_for_os(current_os) {
        steps.push(InstallStep::WriteUninstaller(WriteUninstallerStep {
            path,
            log: None,
        }));
    }

    Ok(steps)
}