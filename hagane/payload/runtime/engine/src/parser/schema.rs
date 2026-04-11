use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Top-level manifest ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerManifest {
    pub app: AppInfo,
    pub theme: Option<ThemeConfig>,
    pub pages: Vec<PageDefinition>,
    pub requirements: Option<Vec<Requirement>>,
    pub components: Option<Vec<Component>>,
    pub steps: Vec<InstallStep>,
    pub uninstall: Option<UninstallConfig>,
    pub silent: Option<SilentConfig>,
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
    /// Default install directory. Supports variables: $PROGRAMFILES, $APPDATA, $LOCALAPPDATA
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
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub custom_html: Option<String>,
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
    Custom(CustomRequirement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRequirement {
    /// e.g. "windows" — future: "linux", "macos"
    pub platform: String,
    /// Minimum Windows build number (e.g. 10240 = Win10, 22000 = Win11)
    pub min_build: Option<u32>,
    /// Human-readable label shown on requirements page
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
    Shortcut(ShortcutStep),
    EnvVar(EnvVarStep),
    Service(ServiceStep),
    RunProgram(RunProgramStep),
    WriteUninstaller(WriteUninstallerStep),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractStep {
    /// Path to the embedded archive name (registered during build)
    pub archive: String,
    pub destination: String,
    /// Only extract if this component id is selected
    pub component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyFileStep {
    pub source: String,
    pub destination: String,
    #[serde(default)]
    pub overwrite: bool,
    pub component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFileStep {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDirStep {
    pub path: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStep {
    pub operation: ServiceOperation,
    pub name: String,
    pub display_name: Option<String>,
    pub executable: Option<String>,
    pub start_type: Option<String>,
    pub description: Option<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteUninstallerStep {
    pub path: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilentConfig {
    /// Install directory override for silent mode
    pub install_dir: Option<String>,
    /// Component IDs to install in silent mode (empty = all required)
    pub components: Option<Vec<String>>,
}