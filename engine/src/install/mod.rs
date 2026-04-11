pub mod extractor;
pub mod files;
pub mod registry_ops;
pub mod rollback;
pub mod services;
pub mod shortcuts;

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::parser::schema::InstallStep;
use rollback::RollbackJournal;

pub struct InstallContext {
    /// Resolved installation directory
    pub install_dir: PathBuf,
    /// Selected component IDs
    pub selected_components: HashSet<String>,
    /// Embedded archives: name → raw compressed bytes
    pub archives: std::collections::HashMap<String, Vec<u8>>,
    /// Temp/backup dir for rollback files
    pub backup_dir: PathBuf,
}

pub struct StepRunner {
    ctx: InstallContext,
    journal: RollbackJournal,
}

impl StepRunner {
    pub fn new(ctx: InstallContext) -> Self {
        let journal = RollbackJournal::new(&ctx.backup_dir);
        Self { ctx, journal }
    }

    /// Execute all steps sequentially. Reports progress via callback.
    /// (current_step_index, total_steps, label)
    pub fn run_all(
        &mut self,
        steps: &[InstallStep],
        on_progress: impl Fn(usize, usize, &str),
    ) -> Result<()> {
        let total = steps.len();
        for (i, step) in steps.iter().enumerate() {
            let label = step_label(step);
            on_progress(i, total, &label);
            log::info!("Step {}/{}: {}", i + 1, total, label);

            if let Err(e) = self.run_step(step) {
                log::error!("Step {} failed: {}", i + 1, e);
                log::warn!("Starting rollback ({} actions to undo)…", self.journal.entry_count());
                let errors = self.journal.rollback();
                for re in &errors {
                    log::error!("Rollback error: {}", re);
                }
                return Err(e.context(format!("Step {}/{} failed: {}", i + 1, total, label)));
            }
        }
        on_progress(total, total, "Done");
        Ok(())
    }

    fn run_step(&mut self, step: &InstallStep) -> Result<()> {
        match step {
            InstallStep::Extract(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                let data = self.ctx.archives.get(&s.archive)
                    .ok_or_else(|| anyhow::anyhow!("Archive '{}' not found in payload", s.archive))?
                    .clone();
                let dest = self.resolve_vars(&s.destination);
                let dest_path = PathBuf::from(&dest);
                extractor::extract_zstd_archive(&data, &dest_path, |_, _| {})
                    .with_context(|| format!("Extract '{}' → '{}'", s.archive, dest))?;
            }

            InstallStep::CopyFile(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                let src = PathBuf::from(self.resolve_vars(&s.source));
                let dst = PathBuf::from(self.resolve_vars(&s.destination));
                files::copy_file(&src, &dst, s.overwrite, &mut self.journal)?;
            }

            InstallStep::DeleteFile(s) => {
                files::delete_file(Path::new(&self.resolve_vars(&s.path)))?;
            }

            InstallStep::CreateDir(s) => {
                let p = PathBuf::from(self.resolve_vars(&s.path));
                files::create_dir(&p, &mut self.journal)?;
            }

            InstallStep::Registry(s) => {
                let resolved_value_data = s.value_data.as_ref().map(|v| self.resolve_value_data(v));
                registry_ops::apply_registry_step(
                    &s.operation,
                    &s.hive,
                    &self.resolve_vars(&s.key),
                    s.value_name.as_deref(),
                    s.value_type.as_ref(),
                    resolved_value_data.as_ref(),
                    &mut self.journal,
                )?;
            }

            InstallStep::Shortcut(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                shortcuts::create_shortcut(
                    &self.resolve_vars(&s.target),
                    &s.location,
                    &s.name,
                    s.description.as_deref(),
                    s.icon.as_deref(),
                    s.arguments.as_deref(),
                    s.working_dir.as_deref(),
                    &mut self.journal,
                )?;
            }

            InstallStep::EnvVar(s) => {
                apply_env_var(
                    &s.name,
                    &self.resolve_vars(&s.value),
                    &s.scope,
                    &s.operation,
                    &mut self.journal,
                )?;
            }

            InstallStep::Service(s) => {
                services::apply_service_step(
                    &s.operation,
                    &s.name,
                    s.display_name.as_deref(),
                    s.executable.as_deref().map(|e| self.resolve_vars(e)).as_deref(),
                    s.start_type.as_deref(),
                    s.description.as_deref(),
                )?;
            }

            InstallStep::RunProgram(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                run_program(
                    &self.resolve_vars(&s.executable),
                    s.arguments.as_deref(),
                    s.wait,
                )?;
            }

            InstallStep::WriteUninstaller(s) => {
                let path = PathBuf::from(self.resolve_vars(&s.path));
                write_uninstaller_stub(&path)?;
            }
        }
        Ok(())
    }

    fn component_active(&self, component: Option<&str>) -> bool {
        match component {
            None => true,
            Some(id) => self.ctx.selected_components.contains(id),
        }
    }

    /// Resolve installer variables in a string:
    /// $INSTDIR, $PROGRAMFILES, $PROGRAMFILES64, $APPDATA, $LOCALAPPDATA, $TEMP, $WINDIR
    pub fn resolve_vars(&self, input: &str) -> String {
        let mut s = input.replace("$INSTDIR", &self.ctx.install_dir.to_string_lossy());

        #[cfg(windows)]
        {
            if let Ok(pf) = std::env::var("ProgramFiles") { s = s.replace("$PROGRAMFILES", &pf); }
            if let Ok(pf64) = std::env::var("ProgramW6432") { s = s.replace("$PROGRAMFILES64", &pf64); }
            if let Ok(ad) = std::env::var("APPDATA")      { s = s.replace("$APPDATA", &ad); }
            if let Ok(la) = std::env::var("LOCALAPPDATA") { s = s.replace("$LOCALAPPDATA", &la); }
            if let Ok(tmp) = std::env::var("TEMP")        { s = s.replace("$TEMP", &tmp); }
            if let Ok(win) = std::env::var("WINDIR")      { s = s.replace("$WINDIR", &win); }
        }
        s
    }

    fn resolve_value_data(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => serde_json::Value::String(self.resolve_vars(s)),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(|v| self.resolve_value_data(v)).collect())
            }
            serde_json::Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (k, v) in map {
                    out.insert(k.clone(), self.resolve_value_data(v));
                }
                serde_json::Value::Object(out)
            }
            _ => value.clone(),
        }
    }

    pub fn into_journal(self) -> RollbackJournal {
        self.journal
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn apply_env_var(
    name: &str,
    value: &str,
    scope: &str,
    operation: &str,
    journal: &mut RollbackJournal,
) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::*;
        use windows::core::PCWSTR;
        let (root, key_path) = match scope {
            "system" => (HKEY_LOCAL_MACHINE, "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"),
            _        => (HKEY_CURRENT_USER,  "Environment"),
        };

        let wide_key: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = HKEY::default();
        unsafe {
            RegOpenKeyExW(root, PCWSTR(wide_key.as_ptr()), 0, KEY_READ | KEY_WRITE, &mut hkey)
                .ok()
                .context("RegOpenKeyExW (env var) failed")?;
        }

        // Read existing value for rollback
        let existing = read_reg_string(hkey, &wide_name);

        let new_value = match operation {
            "append"  => format!("{};{}", existing.as_deref().unwrap_or(""), value),
            "prepend" => format!("{};{}", value, existing.as_deref().unwrap_or("")),
            _         => value.to_string(), // "set"
        };

        let wide_val: Vec<u16> = new_value.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                0,
                REG_EXPAND_SZ,
                Some(std::slice::from_raw_parts(wide_val.as_ptr() as *const u8, wide_val.len() * 2)),
            ).ok().context("RegSetValueExW (env var) failed")?;
            let _ = RegCloseKey(hkey).ok();
        }

        journal.record(rollback::JournalEntry::EnvVarSet {
            name: name.into(),
            scope: scope.into(),
            previous_value: existing,
        });

        // Broadcast WM_SETTINGCHANGE so running apps pick up the change
        broadcast_env_change();
    }
    #[cfg(not(windows))]
    {
        log::warn!("EnvVar step skipped on non-Windows");
    }
    Ok(())
}

#[cfg(windows)]
fn read_reg_string(hkey: windows::Win32::System::Registry::HKEY, wide_name: &[u16]) -> Option<String> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

    let mut buf = vec![0u16; 32767];
    let mut size = (buf.len() * 2) as u32;
    let mut reg_type = REG_VALUE_TYPE::default();

    let res = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        )
    };

    use windows::Win32::Foundation::ERROR_SUCCESS;
    if res.is_ok() {
        let len = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..len]).to_string())
    } else {
        None
    }
}

#[cfg(windows)]
fn broadcast_env_change() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    let env: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(env.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}

fn run_program(executable: &str, arguments: Option<&str>, wait: bool) -> Result<()> {
    let mut cmd = std::process::Command::new(executable);
    if let Some(args) = arguments {
        cmd.args(args.split_whitespace());
    }
    if wait {
        cmd.status().with_context(|| format!("Failed to run: {}", executable))?;
    } else {
        cmd.spawn().with_context(|| format!("Failed to spawn: {}", executable))?;
    }
    Ok(())
}

fn write_uninstaller_stub(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(windows)]
    {
        let current_exe = std::env::current_exe()
            .context("Unable to resolve current installer executable path")?;
        std::fs::copy(&current_exe, path)
            .with_context(|| format!("Failed to write uninstaller at {}", path.display()))?;
        log::info!(
            "Uninstaller written to: {} (source: {})",
            path.display(),
            current_exe.display()
        );
    }

    #[cfg(not(windows))]
    {
        std::fs::write(path, b"#!/bin/sh\necho 'Uninstall is only supported on Windows'\n")?;
        log::info!("Uninstaller stub written to: {}", path.display());
    }

    Ok(())
}

fn step_label(step: &InstallStep) -> String {
    match step {
        InstallStep::Extract(s)       => format!("Extracting {}", s.archive),
        InstallStep::CopyFile(s)      => format!("Copying {}", s.destination),
        InstallStep::DeleteFile(s)    => format!("Deleting {}", s.path),
        InstallStep::CreateDir(s)     => format!("Creating directory {}", s.path),
        InstallStep::Registry(s)      => format!("Writing registry {}\\{}", s.hive, s.key),
        InstallStep::Shortcut(s)      => format!("Creating shortcut '{}'", s.name),
        InstallStep::EnvVar(s)        => format!("Setting environment variable {}", s.name),
        InstallStep::Service(s)       => format!("Service: {:?} '{}'", s.operation, s.name),
        InstallStep::RunProgram(s)    => format!("Running {}", s.executable),
        InstallStep::WriteUninstaller(s) => format!("Writing uninstaller to {}", s.path),
    }
}