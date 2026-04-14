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

    for dir in &install.setup.create_dirs {
        steps.push(InstallStep::CreateDir(CreateDirStep {
            path: dir.clone(),
            log: None,
        }));
    }

    for (component_id, spec) in &install.components {
        steps.push(InstallStep::Extract(ExtractStep {
            archive: spec.archive.clone(),
            destination: spec.target.clone(),
            component: Some(component_id.clone()),
            log: None,
        }));
    }

    if let Some(register_app) = &install.system.register_app {
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
            install_location: register_app
                .install_location
                .clone()
                .unwrap_or_else(|| "{{INSTDIR}}".to_string()),
            version: register_app
                .version
                .clone()
                .unwrap_or_else(|| manifest.app.version.clone()),
            log: None,
        }));
    }

    if let Some(register_uninstall) = &install.system.register_uninstall {
        steps.push(InstallStep::RegisterUninstall(RegisterUninstallStep {
            hive: register_uninstall
                .hive
                .clone()
                .unwrap_or_else(|| "HKLM".to_string()),
            key: register_uninstall.key.clone(),
            display_name: register_uninstall
                .name
                .clone()
                .unwrap_or_else(|| manifest.app.name.clone()),
            display_version: register_uninstall
                .version
                .clone()
                .unwrap_or_else(|| manifest.app.version.clone()),
            publisher: register_uninstall
                .publisher
                .clone()
                .unwrap_or_else(|| manifest.app.publisher.clone()),
            install_location: register_uninstall
                .install_location
                .clone()
                .unwrap_or_else(|| "{{INSTDIR}}".to_string()),
            uninstall_string: register_uninstall
                .uninstall
                .clone()
                .unwrap_or_else(|| "{{INSTDIR}}/uninstall.exe".to_string()),
            estimated_size_kb: register_uninstall.estimated_size_kb,
            no_modify: register_uninstall.no_modify.unwrap_or(true),
            no_repair: register_uninstall.no_repair.unwrap_or(true),
            log: None,
        }));
    }

    if let Some(shortcuts) = &install.system.shortcuts {
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

    if let Some(path) = &install.system.path {
        steps.push(InstallStep::EnvVar(EnvVarStep {
            name: "Path".to_string(),
            value: path.add.clone(),
            scope: path.scope.clone().unwrap_or_else(|| "system".to_string()),
            operation: "append".to_string(),
            component: path.component.clone(),
            log: None,
        }));
    }

    if let Some(hooks) = &install.hooks {
        if let Some(post_install) = &hooks.post_install {
            for hook in post_install {
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

    steps.push(InstallStep::WriteUninstaller(WriteUninstallerStep {
        path: install.finalize.write_uninstaller.clone(),
        log: None,
    }));

    Ok(steps)
}