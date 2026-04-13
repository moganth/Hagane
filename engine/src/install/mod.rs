pub mod extractor;
pub mod files;
pub mod registry_ops;
pub mod rollback;
pub mod services;
pub mod shortcuts;

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crate::parser::schema::{
    InstallStep, LogLevel, LoggingConfig, LoggingMode, RegisterAppStep, RegisterUninstallStep, RegistryOperation,
    RegistryValueType, RunPowerShellStep,
};
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
    /// Optional logging configuration from manifest
    pub logging: Option<LoggingConfig>,
    /// User-declared manifest variables
    pub variables: HashMap<String, String>,
}

pub struct StepRunner {
    ctx: InstallContext,
    journal: RollbackJournal,
    file_logger: Option<FileLogger>,
    manual_only_logging: bool,
}

struct FileLogger {
    path: PathBuf,
    timestamp: bool,
}

impl StepRunner {
    pub fn new(ctx: InstallContext) -> Self {
        let journal = RollbackJournal::new(&ctx.backup_dir);
        let manual_only_logging = matches!(
            ctx.logging.as_ref().and_then(|c| c.mode.clone()),
            Some(LoggingMode::ManualOnly)
        );
        let file_logger = init_file_logger(&ctx);
        Self {
            ctx,
            journal,
            file_logger,
            manual_only_logging,
        }
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
            let progress_label = self.progress_label(step, &label);
            on_progress(i, total, &progress_label);
            log::info!("Step {}/{}: {}", i + 1, total, label);

            if let Err(e) = self.run_step(step) {
                let classified = self.classify_step_error(i + 1, step, &e);
                log::error!("Step {} failed: {}", i + 1, classified);
                let _ = self.write_log_file(LogLevel::Error, &classified);
                log::warn!("Starting rollback ({} actions to undo)…", self.journal.entry_count());
                let errors = self.journal.rollback();
                for re in &errors {
                    log::error!("Rollback error: {}", re);
                    let _ = self.write_log_file(LogLevel::Error, &format!("Rollback error: {}", re));
                }
                return Err(anyhow::anyhow!(classified)
                    .context(format!("Step {}/{} failed: {}", i + 1, total, label)));
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
                let dest = self.resolve_vars_path(&s.destination)?;
                let dest_path = PathBuf::from(&dest);
                extractor::extract_zstd_archive(&data, &dest_path, |_, _| {})
                    .with_context(|| format!("Extract '{}' → '{}'", s.archive, dest))?;
            }

            InstallStep::CopyFile(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                let src = PathBuf::from(self.resolve_vars_path(&s.source)?);
                let dst = PathBuf::from(self.resolve_vars_path(&s.destination)?);
                files::copy_file(&src, &dst, s.overwrite, &mut self.journal)?;
            }

            InstallStep::DeleteFile(s) => {
                files::delete_file(Path::new(&self.resolve_vars_path(&s.path)?))?;
            }

            InstallStep::CreateDir(s) => {
                let p = PathBuf::from(self.resolve_vars_path(&s.path)?);
                files::create_dir(&p, &mut self.journal)?;
            }

            InstallStep::LogUi(s) => {
                let msg = self.resolve_vars(&s.message);
                match s.level.clone().unwrap_or(LogLevel::Info) {
                    LogLevel::Info => log::info!("{}", msg),
                    LogLevel::Warn => log::warn!("{}", msg),
                    LogLevel::Error => log::error!("{}", msg),
                }
            }

            InstallStep::LogFile(s) => {
                let msg = self.resolve_vars(&s.message);
                self.write_log_file(s.level.clone().unwrap_or(LogLevel::Info), &msg)?;
            }

            InstallStep::LogBoth(s) => {
                let msg = self.resolve_vars(&s.message);
                let level = s.level.clone().unwrap_or(LogLevel::Info);
                match level {
                    LogLevel::Info => log::info!("{}", msg),
                    LogLevel::Warn => log::warn!("{}", msg),
                    LogLevel::Error => log::error!("{}", msg),
                }
                self.write_log_file(level, &msg)?;
            }

            InstallStep::Registry(s) => {
                let resolved_value_data = s.value_data.as_ref().map(|v| self.resolve_value_data(v));
                registry_ops::apply_registry_step(
                    &s.operation,
                    &s.hive,
                    &self.resolve_vars_path(&s.key)?,
                    s.value_name.as_deref(),
                    s.value_type.as_ref(),
                    resolved_value_data.as_ref(),
                    &mut self.journal,
                )?;
            }

            InstallStep::RegisterUninstall(s) => {
                self.apply_register_uninstall_step(s)?;
            }

            InstallStep::RegisterApp(s) => {
                self.apply_register_app_step(s)?;
            }

            InstallStep::Shortcut(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                shortcuts::create_shortcut(
                    &self.resolve_vars_path(&s.target)?,
                    &s.location,
                    &s.name,
                    s.description.as_deref(),
                    s.icon.as_deref().map(|v| self.resolve_vars(v)).as_deref(),
                    s.arguments.as_deref(),
                    s.working_dir.as_deref().map(|v| self.resolve_vars(v)).as_deref(),
                    &mut self.journal,
                )?;
            }

            InstallStep::EnvVar(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                apply_env_var(
                    &s.name,
                    &self.resolve_vars_path(&s.value)?,
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
                    s.executable.as_deref().map(|e| self.resolve_vars_path(e)).transpose()?.as_deref(),
                    s.start_type.as_deref(),
                    s.description.as_deref(),
                )?;
            }

            InstallStep::RunProgram(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                let executable = self.resolve_vars_path(&s.executable)?;
                if looks_like_file_path(&executable) && !Path::new(&executable).exists() {
                    return Err(anyhow::anyhow!(
                        "HG-RUN-001: executable not found at '{}'",
                        executable
                    ));
                }
                run_program(
                    &executable,
                    s.arguments.as_deref(),
                    s.wait,
                )?;
            }

            InstallStep::RunPowerShell(s) => {
                if !self.component_active(s.component.as_deref()) { return Ok(()); }
                self.run_powershell(s)?;
            }

            InstallStep::WriteUninstaller(s) => {
                let path = PathBuf::from(self.resolve_vars_path(&s.path)?);
                write_uninstaller_stub(&path)?;
            }
        }
        Ok(())
    }

    fn progress_label(&self, step: &InstallStep, default_label: &str) -> String {
        if !self.manual_only_logging {
            return default_label.to_string();
        }
        match step {
            InstallStep::LogUi(s) => self.resolve_vars(&s.message),
            InstallStep::LogBoth(s) => self.resolve_vars(&s.message),
            _ => String::new(),
        }
    }

    fn run_powershell(&self, step: &RunPowerShellStep) -> Result<()> {
        #[cfg(windows)]
        {
            use std::process::{Command, Stdio};
            use std::time::{Duration, Instant};

            let mut cmd = Command::new("powershell");
            cmd.arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if let Some(script) = &step.script {
                // Stop on non-terminating errors so permissions/command failures
                // produce non-zero exits we can classify reliably.
                let wrapped = format!(
                    "$ErrorActionPreference='Stop'; {}",
                    self.resolve_vars(script)
                );
                cmd.arg("-Command").arg(wrapped);
            } else if let Some(file) = &step.file {
                cmd.arg("-File").arg(self.resolve_vars_path(file)?);
            }

            if let Some(args) = &step.arguments {
                cmd.args(args.split_whitespace());
            }

            if !step.wait {
                cmd.spawn().context("HG-PS-002: failed to spawn powershell")?;
                return Ok(());
            }

            let mut child = cmd.spawn().context("HG-PS-002: failed to spawn powershell")?;

            let output = if let Some(timeout_sec) = step.timeout_sec {
                let deadline = Instant::now() + Duration::from_secs(timeout_sec);
                loop {
                    if let Some(_status) = child.try_wait().context("HG-PS-003: failed while waiting for powershell")? {
                        break child.wait_with_output().context("HG-PS-003: failed to collect powershell output")?;
                    }
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        return Err(anyhow::anyhow!(
                            "HG-PS-004: PowerShell execution timed out after {} seconds",
                            timeout_sec
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            } else {
                child.wait_with_output().context("HG-PS-003: failed to collect powershell output")?
            };

            if step.fail_on_nonzero && !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let reason = stderr.trim();
                if reason.contains("ParserError")
                    || reason.contains("Missing condition")
                    || reason.contains("Unexpected token")
                {
                    return Err(anyhow::anyhow!("HG-PS-001: PowerShell syntax/parse error: {}", reason));
                }
                if reason.contains("is not recognized") || reason.contains("CommandNotFoundException") {
                    return Err(anyhow::anyhow!("HG-PS-002: PowerShell command not found: {}", reason));
                }
                if reason.contains("running scripts is disabled")
                    || reason.contains("UnauthorizedAccess")
                    || reason.contains("Access is denied")
                    || reason.contains("Access to the path")
                    || reason.contains("PSSecurityException")
                {
                    return Err(anyhow::anyhow!("HG-PS-005: PowerShell blocked by policy/access rules: {}", reason));
                }
                return Err(anyhow::anyhow!(
                    "HG-PS-003: PowerShell returned non-zero exit code {:?}: {}",
                    output.status.code(),
                    reason
                ));
            }
        }

        #[cfg(not(windows))]
        {
            let _ = step;
            return Err(anyhow::anyhow!("HG-PS-002: run_powershell is only supported on Windows"));
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
        resolve_vars_with_install_dir(input, &self.ctx.install_dir, &self.ctx.variables)
    }

    fn resolve_vars_path(&self, input: &str) -> Result<String> {
        let resolved = self.resolve_vars(input);
        if let Some(token) = unresolved_token(&resolved) {
            return Err(anyhow::anyhow!(
                "HG-VAR-001: unresolved variable '{}' in '{}'",
                token,
                input
            ));
        }
        Ok(resolved)
    }

    fn write_log_file(&self, level: LogLevel, message: &str) -> Result<()> {
        let Some(logger) = &self.file_logger else {
            return Err(anyhow::anyhow!(
                "HG-YAML-001: logging.path and logging.file_name must be configured for action 'log_file'"
            ));
        };

        let ts = if logger.timestamp {
            format!("[{}] ", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        } else {
            String::new()
        };
        let level = match level {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        let line = format!("{}[{}] {}\n", ts, level, message);
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&logger.path)
            .with_context(|| format!("HG-EXTRACT-002: cannot open log file {}", logger.path.display()))?;
        f.write_all(line.as_bytes())
            .with_context(|| format!("HG-EXTRACT-002: cannot write log file {}", logger.path.display()))?;
        Ok(())
    }

    fn classify_step_error(&self, step_index: usize, step: &InstallStep, err: &anyhow::Error) -> String {
        let raw = format!("{:#}", err);
        let lower = raw.to_lowercase();

        // Preserve explicit code-originated failures from helpers.
        if raw.contains("HG-VAR-001") {
            return format!(
                "[ERROR] HG-VAR-001 step={} action={} field=path value={} reason=\"{}\" fix=\"Use supported variables ($INSTDIR, $PROGRAMFILES, $PROGRAMFILES64, $APPDATA, $LOCALAPPDATA, $TEMP, $WINDIR).\"",
                step_index,
                step_action(step),
                step_label(step),
                first_reason_line(&raw),
            );
        }

        let permission_related = lower.contains("access is denied")
            || lower.contains("permission denied")
            || lower.contains("os error 5")
            || lower.contains("unauthorized")
            || lower.contains("privilege")
            || lower.contains("elevation");

        let (code, field, value, reason, fix) = match step {
            InstallStep::Extract(s) => {
                if raw.contains("not found in payload") {
                    (
                        "HG-EXTRACT-001",
                        "archive",
                        s.archive.clone(),
                        format!("archive '{}' is missing from embedded payload", s.archive),
                        "Run hagane build again and ensure the archive source folder exists near installer.yaml.",
                    )
                } else {
                    (
                        "HG-EXTRACT-002",
                        "destination",
                        s.destination.clone(),
                        first_reason_line(&raw),
                        "Check destination path permissions and ensure no file lock prevents extraction. If writing into protected locations, run installer elevated.",
                    )
                }
            }
            InstallStep::CopyFile(s) => (
                "HG-COPY-001",
                "source",
                s.source.clone(),
                first_reason_line(&raw),
                "Verify the source file exists and the path resolves correctly.",
            ),
            InstallStep::Registry(s) => {
                let likely_hklm_permission = s.hive == "HKLM"
                    && (lower.contains("regcreatekeyexw failed") || lower.contains("regopenkeyexw failed"));
                if permission_related || likely_hklm_permission {
                    (
                        "HG-REG-002",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Registry write requires elevated permission. Run installer as Administrator or use HKCU for user-scope settings.",
                    )
                } else {
                    (
                        "HG-REG-001",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Validate hive/key/value_type and ensure the registry path is valid.",
                    )
                }
            }
            InstallStep::RegisterUninstall(s) => {
                let likely_hklm_permission = s.hive == "HKLM"
                    && (lower.contains("regcreatekeyexw failed") || lower.contains("regopenkeyexw failed"));
                if permission_related || likely_hklm_permission {
                    (
                        "HG-REG-002",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Registry write requires elevated permission. Run installer as Administrator or use HKCU for user-scope settings.",
                    )
                } else {
                    (
                        "HG-REG-001",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Validate register_app key and values, and ensure the registry path is valid.",
                    )
                }
            }
            InstallStep::RegisterApp(s) => {
                let likely_hklm_permission = s.hive == "HKLM"
                    && (lower.contains("regcreatekeyexw failed") || lower.contains("regopenkeyexw failed"));
                if permission_related || likely_hklm_permission {
                    (
                        "HG-REG-002",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Registry write requires elevated permission. Run installer as Administrator or use HKCU for user-scope settings.",
                    )
                } else {
                    (
                        "HG-REG-001",
                        "key",
                        format!("{}\\{}", s.hive, s.key),
                        first_reason_line(&raw),
                        "Validate register_app key and values, and ensure the registry path is valid.",
                    )
                }
            }
            InstallStep::EnvVar(s) => (
                "HG-ENV-001",
                "operation",
                format!("scope={}, operation={}", s.scope, s.operation),
                first_reason_line(&raw),
                "Use scope user/system and operation set/append/prepend. For system scope, run installer elevated.",
            ),
            InstallStep::RunProgram(s) => {
                if lower.contains("not found")
                    || lower.contains("cannot find")
                    || lower.contains("os error 2")
                {
                    (
                        "HG-RUN-001",
                        "executable",
                        s.executable.clone(),
                        first_reason_line(&raw),
                        "Check executable path and confirm file exists after extract/copy steps.",
                    )
                } else {
                    (
                        "HG-RUN-002",
                        "executable",
                        s.executable.clone(),
                        first_reason_line(&raw),
                        "Review arguments and the target program output; ensure dependencies are present.",
                    )
                }
            }
            InstallStep::RunPowerShell(s) => {
                let value = s.file.clone().or_else(|| s.script.clone()).unwrap_or_default();
                if raw.contains("HG-PS-001") {
                    ("HG-PS-001", "script", value, first_reason_line(&raw), "Fix PowerShell script syntax and test it manually with powershell -NoProfile.")
                } else if raw.contains("HG-PS-002") {
                    ("HG-PS-002", "script", value, first_reason_line(&raw), "Ensure powershell and all referenced commands are available in PATH.")
                } else if raw.contains("HG-PS-004") {
                    ("HG-PS-004", "timeout_sec", value, first_reason_line(&raw), "Increase timeout_sec or optimize the script.")
                } else if raw.contains("HG-PS-005") {
                    ("HG-PS-005", "script", value, first_reason_line(&raw), "PowerShell requires elevated permission or policy changes. Run as Administrator or adjust ExecutionPolicy/signing.")
                } else {
                    ("HG-PS-003", "script", value, first_reason_line(&raw), "Check script output and exit code handling.")
                }
            }
            _ => (
                "HG-RUN-002",
                "step",
                step_label(step),
                first_reason_line(&raw),
                "Review the failing step configuration and referenced resources.",
            ),
        };

        format!(
            "[ERROR] {} step={} action={} field={} value={} reason=\"{}\" fix=\"{}\"",
            code,
            step_index,
            step_action(step),
            field,
            value,
            reason,
            fix,
        )
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

    fn apply_register_uninstall_step(&mut self, step: &RegisterUninstallStep) -> Result<()> {
        let key = self.resolve_vars_path(&step.key)?;

        let display_name = serde_json::Value::String(self.resolve_vars(&step.display_name));
        let display_version = serde_json::Value::String(self.resolve_vars(&step.display_version));
        let publisher = serde_json::Value::String(self.resolve_vars(&step.publisher));
        let install_location = serde_json::Value::String(self.resolve_vars_path(&step.install_location)?);
        let uninstall_string = serde_json::Value::String(self.resolve_vars_path(&step.uninstall_string)?);

        let write = |value_name: &str,
                     value_type: RegistryValueType,
                     value_data: &serde_json::Value,
                     journal: &mut RollbackJournal|
         -> Result<()> {
            registry_ops::apply_registry_step(
                &RegistryOperation::Write,
                &step.hive,
                &key,
                Some(value_name),
                Some(&value_type),
                Some(value_data),
                journal,
            )
        };

        write("DisplayName", RegistryValueType::Sz, &display_name, &mut self.journal)?;
        write("DisplayVersion", RegistryValueType::Sz, &display_version, &mut self.journal)?;
        write("Publisher", RegistryValueType::Sz, &publisher, &mut self.journal)?;
        write("InstallLocation", RegistryValueType::Sz, &install_location, &mut self.journal)?;
        write("UninstallString", RegistryValueType::Sz, &uninstall_string, &mut self.journal)?;

        if let Some(kb) = step.estimated_size_kb {
            let v = serde_json::Value::Number(serde_json::Number::from(kb));
            write("EstimatedSize", RegistryValueType::Dword, &v, &mut self.journal)?;
        }

        let no_modify = serde_json::Value::Number(serde_json::Number::from(if step.no_modify { 1 } else { 0 }));
        write("NoModify", RegistryValueType::Dword, &no_modify, &mut self.journal)?;

        let no_repair = serde_json::Value::Number(serde_json::Number::from(if step.no_repair { 1 } else { 0 }));
        write("NoRepair", RegistryValueType::Dword, &no_repair, &mut self.journal)?;

        Ok(())
    }

    fn apply_register_app_step(&mut self, step: &RegisterAppStep) -> Result<()> {
        let key = self.resolve_vars_path(&step.key)?;

        let install_location = serde_json::Value::String(self.resolve_vars_path(&step.install_location)?);
        registry_ops::apply_registry_step(
            &RegistryOperation::Write,
            &step.hive,
            &key,
            Some("InstallDir"),
            Some(&RegistryValueType::Sz),
            Some(&install_location),
            &mut self.journal,
        )?;

        let version = serde_json::Value::String(self.resolve_vars(&step.version));
        registry_ops::apply_registry_step(
            &RegistryOperation::Write,
            &step.hive,
            &key,
            Some("Version"),
            Some(&RegistryValueType::Sz),
            Some(&version),
            &mut self.journal,
        )?;

        Ok(())
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
        let status = cmd.status().with_context(|| format!("Failed to run: {}", executable))?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "HG-RUN-002: process '{}' returned non-zero exit code {:?}",
                executable,
                status.code()
            ));
        }
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
        InstallStep::LogUi(s)         => s.message.clone(),
        InstallStep::LogFile(_)       => "Writing installer log".to_string(),
        InstallStep::LogBoth(s)       => s.message.clone(),
        InstallStep::Registry(s)          => format!("Writing registry {}\\{}", s.hive, s.key),
        InstallStep::RegisterUninstall(s) => format!("Registering uninstall entry {}\\{}", s.hive, s.key),
        InstallStep::RegisterApp(s)       => format!("Registering app settings {}\\{}", s.hive, s.key),
        InstallStep::Shortcut(s)      => format!("Creating shortcut '{}'", s.name),
        InstallStep::EnvVar(s)        => format!("Setting environment variable {}", s.name),
        InstallStep::Service(s)       => format!("Service: {:?} '{}'", s.operation, s.name),
        InstallStep::RunProgram(s)    => format!("Running {}", s.executable),
        InstallStep::RunPowerShell(_) => "Running PowerShell".to_string(),
        InstallStep::WriteUninstaller(s) => format!("Writing uninstaller to {}", s.path),
    }
}

fn step_action(step: &InstallStep) -> &'static str {
    match step {
        InstallStep::Extract(_) => "extract",
        InstallStep::CopyFile(_) => "copy_file",
        InstallStep::DeleteFile(_) => "delete_file",
        InstallStep::CreateDir(_) => "create_dir",
        InstallStep::LogUi(_) => "log_ui",
        InstallStep::LogFile(_) => "log_file",
        InstallStep::LogBoth(_) => "log_both",
        InstallStep::Registry(_) => "registry",
        InstallStep::RegisterUninstall(_) => "register_uninstall",
        InstallStep::RegisterApp(_) => "register_app",
        InstallStep::Shortcut(_) => "shortcut",
        InstallStep::EnvVar(_) => "env_var",
        InstallStep::Service(_) => "service",
        InstallStep::RunProgram(_) => "run_program",
        InstallStep::RunPowerShell(_) => "run_powershell",
        InstallStep::WriteUninstaller(_) => "write_uninstaller",
    }
}

fn resolve_vars_with_install_dir(
    input: &str,
    install_dir: &Path,
    declared_vars: &HashMap<String, String>,
) -> String {
    let mut s = input.to_string();

    // Resolve user-declared variables first so they can still contain built-ins
    // that are substituted in the next pass.
    for _ in 0..10 {
        let before = s.clone();
        for (key, value) in declared_vars {
            if let Some(normalized) = normalize_declared_var_key(key) {
                let token = format!("${}", normalized);
                s = s.replace(&token, value);
            }
        }
        if s == before {
            break;
        }
    }

    // Resolve install dir after declared variables so values like
    // LOG_DIR="$INSTDIR\\logs" expand correctly.
    s = s.replace("$INSTDIR", &install_dir.to_string_lossy());

    #[cfg(windows)]
    {
        if let Ok(pf64) = std::env::var("ProgramW6432") { s = s.replace("$PROGRAMFILES64", &pf64); }
        if let Ok(pf) = std::env::var("ProgramFiles") { s = s.replace("$PROGRAMFILES", &pf); }
        if let Ok(ad) = std::env::var("APPDATA")      { s = s.replace("$APPDATA", &ad); }
        if let Ok(la) = std::env::var("LOCALAPPDATA") { s = s.replace("$LOCALAPPDATA", &la); }
        if let Ok(tmp) = std::env::var("TEMP")        { s = s.replace("$TEMP", &tmp); }
        if let Ok(win) = std::env::var("WINDIR")      { s = s.replace("$WINDIR", &win); }
    }
    s
}

fn unresolved_token(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i;
            i += 1;
            let mut end = i;
            while end < bytes.len() {
                let c = bytes[end] as char;
                if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > i {
                return Some(value[start..end].to_string());
            }
        }
        i += 1;
    }
    None
}

fn init_file_logger(ctx: &InstallContext) -> Option<FileLogger> {
    let config = ctx.logging.as_ref()?;
    let path_raw = config.path.as_ref()?;
    let file_name = config.file_name.as_ref()?;

    let dir = PathBuf::from(resolve_vars_with_install_dir(path_raw, &ctx.install_dir, &ctx.variables));
    let mut file_path = dir.clone();
    file_path.push(file_name);

    if let Some(parent) = file_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::error!("Failed to create log directory '{}': {}", parent.display(), e);
            return None;
        }
    }

    Some(FileLogger {
        path: file_path,
        timestamp: config.timestamp.unwrap_or(true),
    })
}

fn first_reason_line(raw: &str) -> String {
    raw.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or(raw)
        .replace('"', "'")
}

fn looks_like_file_path(value: &str) -> bool {
    value.contains('\\') || value.contains('/') || value.contains(':') || value.starts_with('.')
}

fn normalize_declared_var_key(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.trim_start_matches('$');
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.to_string())
}