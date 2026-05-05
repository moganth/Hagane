use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// ── Top-level manifest ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerManifest {
    /// Optional build configuration block (must appear first in YAML).
    pub build: Option<BuildConfig>,
    pub app: AppInfo,
    pub variables: Option<VariablesConfig>,
    pub theme: Option<ThemeConfig>,
    pub pages: Vec<PageDefinition>,
    pub requirements: Option<Vec<Requirement>>,
    pub components: Option<Vec<Component>>,
    pub logging: Option<LoggingConfig>,
    pub install: InstallDsl,
    #[serde(default, rename = "steps")]
    pub legacy_steps: Option<Vec<InstallStep>>,
    #[serde(skip_deserializing, default)]
    pub steps: Vec<InstallStep>,
    pub uninstall: Option<UninstallConfig>,
    pub silent: Option<SilentConfig>,
}

// ── Build config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Target OSes: "windows", "linux", or both.
    pub os: OsTargets,
    /// Output formats per platform.
    pub outputs: Option<BuildOutputs>,
}

/// Accept either a single string ("windows") or a list (["windows","linux"]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OsTargets {
    Single(String),
    Multi(Vec<String>),
}

impl OsTargets {
    pub fn contains(&self, target: &str) -> bool {
        match self {
            OsTargets::Single(s) => s == target,
            OsTargets::Multi(v) => v.iter().any(|s| s == target),
        }
    }
    pub fn targets(&self) -> Vec<&str> {
        match self {
            OsTargets::Single(s) => vec![s.as_str()],
            OsTargets::Multi(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildOutputs {
    pub windows: Option<Vec<String>>,
    pub linux: Option<Vec<String>>,
}

// ── Variables ─────────────────────────────────────────────────────────────────

/// Variables block — flat map plus optional per-platform overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VariablesConfig {
    /// Platform-specific variable sub-blocks.
    pub platform: Option<PlatformVariables>,
    /// All other flat key=value entries captured via flatten.
    #[serde(flatten)]
    pub shared: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformVariables {
    pub windows: Option<HashMap<String, String>>,
    pub linux: Option<HashMap<String, String>>,
}

impl VariablesConfig {
    /// Resolve to a flat string map for the given runtime OS.
    /// Platform-specific values override shared ones.
    pub fn resolve_for_os(&self, os: &str) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = self
            .shared
            .iter()
            .filter_map(|(k, v)| {
                if k == "platform" { return None; }
                let s = match v {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
                };
                Some((k.clone(), s))
            })
            .collect();
        if let Some(platform) = &self.platform {
            let overrides = match os {
                "windows" => platform.windows.as_ref(),
                "linux"   => platform.linux.as_ref(),
                _ => None,
            };
            if let Some(map) = overrides {
                for (k, v) in map {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallDsl {
    pub setup: InstallSetupDsl,
    pub components: BTreeMap<String, InstallComponentDsl>,
    /// Flat (legacy/Windows-only) system block — used when no platform sub-keys present.
    #[serde(default)]
    pub system: InstallSystemFlat,
    pub hooks: Option<InstallHooksDsl>,
    pub finalize: InstallFinalizePlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSetupDsl {
    pub create_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallComponentDsl {
    pub archive: String,
    pub target: String,
}

/// Accepts either the old flat layout or the new platform-split layout:
///   system:
///     register_app: ...          ← flat (Windows-only)
///
///   system:
///     windows:
///       register_app: ...
///     linux:
///       desktop_entry: ...
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallSystemFlat {
    // ── flat (backwards-compatible) ──
    pub register_app: Option<InstallRegisterAppDsl>,
    pub register_uninstall: Option<InstallRegisterUninstallDsl>,
    pub shortcuts: Option<Vec<InstallShortcutDsl>>,
    pub path: Option<InstallPathDsl>,
    // ── platform sub-keys ──
    pub windows: Option<InstallSystemDsl>,
    pub linux: Option<InstallSystemLinuxDsl>,
}

impl InstallSystemFlat {
    /// Return the Windows-effective system config: prefer explicit windows sub-block,
    /// fall back to the flat fields (backwards compatibility).
    pub fn windows_effective(&self) -> InstallSystemDsl {
        if let Some(w) = &self.windows { return w.clone(); }
        InstallSystemDsl {
            register_app:       self.register_app.clone(),
            register_uninstall: self.register_uninstall.clone(),
            shortcuts:          self.shortcuts.clone(),
            path:               self.path.clone(),
        }
    }

    pub fn linux_effective(&self) -> Option<&InstallSystemLinuxDsl> {
        self.linux.as_ref()
    }
}

/// Windows-specific system integration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallSystemDsl {
    pub register_app: Option<InstallRegisterAppDsl>,
    pub register_uninstall: Option<InstallRegisterUninstallDsl>,
    pub shortcuts: Option<Vec<InstallShortcutDsl>>,
    pub path: Option<InstallPathDsl>,
}

/// Linux-specific system integration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallSystemLinuxDsl {
    /// Writes an INI/JSON/TOML config file (replaces registry for Linux).
    pub config: Option<LinuxConfigDsl>,
    /// Writes a JSON uninstall manifest (replaces Add/Remove Programs).
    pub uninstall_manifest: Option<LinuxUninstallManifestDsl>,
    /// Creates an XDG .desktop file (replaces .lnk shortcuts).
    pub desktop_entry: Option<LinuxDesktopEntryDsl>,
    /// Appends to PATH via shell profile (replaces registry PATH write).
    /// Accepts a single entry or a list so both user and system scopes
    /// can be defined simultaneously (each gated on its own component).
    #[serde(default, deserialize_with = "deserialize_path_list")]
    pub path: Vec<InstallPathDsl>,
}

/// Deserialises `path:` as either a single `InstallPathDsl` object or a list
/// of them, so existing single-entry YAMLs continue to work unchanged.
pub fn deserialize_path_list<'de, D>(d: D) -> Result<Vec<InstallPathDsl>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(InstallPathDsl),
        Many(Vec<InstallPathDsl>),
    }
    match OneOrMany::deserialize(d)? {
        OneOrMany::One(p)  => Ok(vec![p]),
        OneOrMany::Many(v) => Ok(v),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxConfigDsl {
    pub path: String,
    #[serde(default = "default_ini")]
    pub format: String,
    pub entries: Vec<LinuxKvEntry>,
}

fn default_ini() -> String { "ini".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxKvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxUninstallManifestDsl {
    pub path: String,
    pub entries: Vec<LinuxKvEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxDesktopEntryDsl {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub categories: Option<String>,
    #[serde(default)]
    pub terminal: bool,
    /// "user" → ~/.local/share/applications, "system" → /usr/share/applications
    #[serde(default = "default_user")]
    pub location: String,
    pub component: Option<String>,
}

fn default_user() -> String { "user".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRegisterAppDsl {
    pub key: Option<String>,
    pub hive: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRegisterUninstallDsl {
    pub key: String,
    pub hive: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub uninstall: Option<String>,
    pub estimated_size_kb: Option<u32>,
    pub no_modify: Option<bool>,
    pub no_repair: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallShortcutDsl {
    pub name: String,
    pub target: String,
    pub location: ShortcutLocation,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPathDsl {
    pub add: String,
    pub scope: Option<String>,
    pub component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallHooksDsl {
    pub post_install: Option<Vec<InstallHookStepDsl>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallHookStepDsl {
    pub run: InstallRunHookDsl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRunHookDsl {
    pub command: String,
    pub shell: InstallHookShell,
    /// If set, this hook only runs on the specified OS ("windows" or "linux").
    pub platform: Option<String>,
    #[serde(default = "default_true")]
    pub wait: bool,
    #[serde(default = "default_true")]
    pub fail_on_nonzero: bool,
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallHookShell {
    Powershell,
    Bash,
    Program,
}

/// Accepts either a flat string (legacy) or platform sub-blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InstallFinalizePlatform {
    /// Legacy flat form: `write_uninstaller: "..."`
    Flat(InstallFinalizeDsl),
    /// New platform-split form
    Platform(InstallFinalizeSplit),
}

impl InstallFinalizePlatform {
    pub fn write_uninstaller_for_os(&self, os: &str) -> Option<String> {
        match self {
            InstallFinalizePlatform::Flat(f) => Some(f.write_uninstaller.clone()),
            InstallFinalizePlatform::Platform(p) => match os {
                "windows" => p.windows.as_ref().map(|w| w.write_uninstaller.clone()),
                "linux"   => p.linux.as_ref().map(|l| l.write_uninstaller.clone()),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallFinalizeDsl {
    pub write_uninstaller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallFinalizeSplit {
    pub windows: Option<InstallFinalizeDsl>,
    pub linux: Option<InstallFinalizeDsl>,
}

// ── Logging ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub mode: Option<LoggingMode>,
    pub path: Option<String>,
    pub file_name: Option<String>,
    pub timestamp: Option<bool>,
    pub include_raw_os_error: Option<bool>,
    pub slow_step_warn_sec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoggingMode {
    Auto,
    ManualOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

// ── App metadata ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub description: Option<String>,
    pub website: Option<String>,
    pub support_url: Option<String>,
    pub logo: Option<String>,
    pub banner: Option<String>,
    pub icon: Option<String>,
    /// Default install directory. Supports variables in either form:
    /// $PROGRAMFILES / {{PROGRAMFILES}}, $APPDATA / {{APPDATA}}, $LOCALAPPDATA / {{LOCALAPPDATA}}
    pub default_install_dir: Option<String>,
    /// Registry key for uninstall entry
    pub registry_key: Option<String>,
    /// Require administrator elevation
    #[serde(default = "default_true")]
    pub require_admin: bool,
}

fn default_true() -> bool { true }

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub preset: Option<String>,
    pub accent_color: Option<String>,
    pub accent_dark_color: Option<String>,
    pub accent_light_color: Option<String>,
    pub background_color: Option<String>,
    pub surface_color: Option<String>,
    pub text_color: Option<String>,
    pub text_muted_color: Option<String>,
    pub border_color: Option<String>,
    pub success_color: Option<String>,
    pub success_bg_color: Option<String>,
    pub error_color: Option<String>,
    pub error_bg_color: Option<String>,
    pub progress_color: Option<String>,
    pub progress_light_color: Option<String>,
    pub font_family: Option<String>,
    pub border_radius: Option<u8>,
    pub banner_position: Option<BannerPosition>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BannerPosition {
    Top,
    Left,
    None,
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDefinition {
    #[serde(rename = "type")]
    pub page_type: PageType,
    pub id: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub custom_html: Option<String>,
    pub widgets: Option<Vec<CustomWidget>>,
    pub interactive: Option<bool>,
    /// Extra key-value data passed to the page template
    pub data: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageType {
    Welcome,
    License,
    Requirements,
    InstallDir,
    Components,
    UserInfo,
    Summary,
    Install,
    Finish,
    Error,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomWidget {
    Label(CustomLabelWidget),
    TextInput(CustomTextWidget),
    MultilineInput(CustomTextWidget),
    Checkbox(CustomCheckboxWidget),
    RadioGroup(CustomChoiceWidget),
    Dropdown(CustomChoiceWidget),
    FolderPicker(CustomPathWidget),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomLabelWidget {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTextWidget {
    pub id: String,
    pub label: String,
    pub bind_to: Option<String>,
    pub placeholder: Option<String>,
    pub default: Option<String>,
    pub help_text: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCheckboxWidget {
    pub id: String,
    pub label: String,
    pub bind_to: Option<String>,
    #[serde(default)]
    pub default: bool,
    pub help_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChoiceWidget {
    pub id: String,
    pub label: String,
    pub bind_to: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub default: Option<String>,
    pub help_text: Option<String>,
    pub options: Vec<CustomChoiceOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChoiceOption {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPathWidget {
    pub id: String,
    pub label: String,
    pub bind_to: Option<String>,
    pub default: Option<String>,
    pub placeholder: Option<String>,
    pub browse_title: Option<String>,
    pub help_text: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub must_exist: bool,
}

// ── Requirements ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Requirement {
    Os(OsRequirement),
    Ram(RamRequirement),
    Disk(DiskRequirement),
    Dotnet(DotnetRequirement),
    VcRedist(VcRedistRequirement),
    Package(PackageRequirement),
    Custom(CustomRequirement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRequirement {
    /// "windows" or "linux"
    pub platform: String,
    /// Windows: minimum build number (e.g. 18362 = Win10 1903)
    pub min_build: Option<u32>,
    /// Linux: minimum distro version string (e.g. "20.04" for Ubuntu)
    pub min_version: Option<String>,
    /// Linux: allowed distro names (e.g. ["ubuntu", "debian"])
    pub distros: Option<Vec<String>>,
    /// Human-readable label shown on requirements page
    pub label: Option<String>,
}

/// Linux-only: checks for a system package via dpkg/rpm/pacman.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRequirement {
    /// Only runs on this platform ("linux" expected).
    pub platform: Option<String>,
    /// Package name to check (e.g. "libgtk-3-0").
    pub name: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamRequirement {
    /// Minimum RAM in megabytes
    pub min_mb: u64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskRequirement {
    /// Required free space in megabytes
    pub min_mb: u64,
    /// Drive/path to check (default: install dir drive)
    pub path: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotnetRequirement {
    /// Minimum .NET Framework version e.g. "4.8"
    pub min_version: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcRedistRequirement {
    /// e.g. "2015", "2017", "2019", "2022"
    pub year: String,
    /// "x86", "x64", "arm64"
    pub arch: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRequirement {
    pub id: String,
    pub label: String,
    /// PowerShell expression — last resort only, prefer native checks
    pub check_script: Option<String>,
}

// ── Components (optional features) ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Size in MB shown to the user
    pub size_mb: Option<u64>,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_true")]
    pub selected: bool,
    /// Only install this component if the given component id is also selected
    pub depends_on: Option<Vec<String>>,
}

// ── Install steps ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InstallStep {
    Extract(ExtractStep),
    CopyFile(CopyFileStep),
    DeleteFile(DeleteFileStep),
    CreateDir(CreateDirStep),
    Registry(RegistryStep),
    RegisterUninstall(RegisterUninstallStep),
    RegisterApp(RegisterAppStep),
    Shortcut(ShortcutStep),
    EnvVar(EnvVarStep),
    Service(ServiceStep),
    RunProgram(RunProgramStep),
    #[serde(rename = "run_powershell", alias = "run_power_shell")]
    RunPowerShell(RunPowerShellStep),
    RunBash(RunBashStep),
    WriteUninstaller(WriteUninstallerStep),
    WriteLinuxConfig(WriteLinuxConfigStep),
    WriteDesktopEntry(WriteDesktopEntryStep),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineLogSpec {
    pub both: Option<String>,
    pub ui: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractStep {
    /// Path to the embedded archive name (registered during build)
    pub archive: String,
    pub destination: String,
    /// Only extract if this component id is selected
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyFileStep {
    pub source: String,
    pub destination: String,
    #[serde(default)]
    pub overwrite: bool,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFileStep {
    pub path: String,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDirStep {
    pub path: String,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStep {
    pub operation: RegistryOperation,
    /// "HKLM", "HKCU", "HKCR", "HKU", "HKCC"
    pub hive: String,
    pub key: String,
    pub value_name: Option<String>,
    pub value_type: Option<RegistryValueType>,
    pub value_data: Option<serde_json::Value>,
    pub log: Option<InlineLogSpec>,
}

/// High-level uninstall registration helper.
/// Expands internally to standard uninstall registry values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterUninstallStep {
    /// "HKLM", "HKCU", "HKCR", "HKU", "HKCC"
    pub hive: String,
    /// Full uninstall key path under the chosen hive.
    pub key: String,
    #[serde(alias = "name")]
    pub display_name: String,
    #[serde(alias = "version")]
    pub display_version: String,
    pub publisher: String,
    #[serde(alias = "inst_loc", alias = "Inst_loc")]
    pub install_location: String,
    #[serde(alias = "uninstall")]
    pub uninstall_string: String,
    pub estimated_size_kb: Option<u32>,
    #[serde(default = "default_true")]
    pub no_modify: bool,
    #[serde(default = "default_true")]
    pub no_repair: bool,
    pub log: Option<InlineLogSpec>,
}

/// High-level app registry helper.
/// Writes standard app registry values under a single app key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAppStep {
    /// "HKLM", "HKCU", "HKCR", "HKU", "HKCC"
    pub hive: String,
    pub key: String,
    #[serde(alias = "inst_loc", alias = "Inst_loc")]
    pub install_location: String,
    pub version: String,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistryOperation {
    Write,
    Delete,
    CreateKey,
    DeleteKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RegistryValueType {
    Sz,
    ExpandSz,
    Dword,
    Qword,
    MultiSz,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutStep {
    pub target: String,
    pub location: ShortcutLocation,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutLocation {
    Desktop,
    StartMenu,
    Startup,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarStep {
    pub name: String,
    pub value: String,
    /// "user" or "system"
    pub scope: String,
    /// "set", "append", "prepend"
    pub operation: String,
    /// Only apply this env var step if the given component id is selected
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStep {
    pub operation: ServiceOperation,
    pub name: String,
    pub display_name: Option<String>,
    pub executable: Option<String>,
    pub start_type: Option<String>,
    pub description: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceOperation {
    Install,
    Start,
    Stop,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunProgramStep {
    pub executable: String,
    pub arguments: Option<String>,
    /// Wait for the process to exit before continuing
    #[serde(default = "default_true")]
    pub wait: bool,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPowerShellStep {
    pub script: Option<String>,
    pub file: Option<String>,
    pub arguments: Option<String>,
    /// Wait for the process to exit before continuing
    #[serde(default = "default_true")]
    pub wait: bool,
    /// If true, non-zero exit codes fail the installation
    #[serde(default = "default_true")]
    pub fail_on_nonzero: bool,
    /// Timeout in seconds for wait=true mode
    pub timeout_sec: Option<u64>,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteUninstallerStep {
    pub path: String,
    pub log: Option<InlineLogSpec>,
}

/// Writes a config file (INI/JSON/TOML) to disk — Linux equivalent of registry writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteLinuxConfigStep {
    pub path: String,
    /// "ini", "json", or "toml"
    pub format: String,
    /// Ordered key-value pairs to write.
    pub entries: Vec<(String, String)>,
    pub log: Option<InlineLogSpec>,
}

/// Writes an XDG .desktop file — Linux equivalent of .lnk shortcuts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteDesktopEntryStep {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub categories: Option<String>,
    pub terminal: bool,
    /// "user" or "system"
    pub location: String,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

/// Runs a bash/sh script — Linux equivalent of RunPowerShell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBashStep {
    /// Inline script body.
    pub script: Option<String>,
    /// Path to a script file.
    pub file: Option<String>,
    pub wait: bool,
    pub fail_on_nonzero: bool,
    pub timeout_sec: Option<u64>,
    pub component: Option<String>,
    pub log: Option<InlineLogSpec>,
}

// ── Uninstall config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallConfig {
    pub display_name: Option<String>,
    pub publisher: Option<String>,
    /// Steps to run during uninstall (in addition to auto-reversal)
    pub extra_steps: Option<Vec<InstallStep>>,
}

// ── Silent install config ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilentConfig {
    /// Accepted silent-mode flags. Defaults: [\"/S\", \"--silent\", \"-s\"]
    pub flags: Option<Vec<String>>,
    /// Install directory override for silent mode
    pub install_dir: Option<String>,
    /// Component IDs to install in silent mode (empty = all required)
    pub components: Option<Vec<String>>,
}