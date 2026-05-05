use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::parser::schema::{CustomWidget, InstallerManifest, PageDefinition, PageType};
use crate::requirements::CheckResult;

// ── Pages ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
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
    Custom(String),
}

impl From<&PageType> for Page {
    fn from(p: &PageType) -> Self {
        match p {
            PageType::Welcome      => Page::Welcome,
            PageType::License      => Page::License,
            PageType::Requirements => Page::Requirements,
            PageType::InstallDir   => Page::InstallDir,
            PageType::Components   => Page::Components,
            PageType::UserInfo     => Page::UserInfo,
            PageType::Summary      => Page::Summary,
            PageType::Install      => Page::Install,
            PageType::Finish       => Page::Finish,
            PageType::Error        => Page::Error,
            PageType::Custom       => Page::Custom("custom".to_string()),
        }
    }
}

// ── Install progress ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub current_step: usize,
    pub total_steps: usize,
    pub current_label: String,
    pub percent: u8,
    pub log: Vec<String>,
}

impl InstallProgress {
    pub fn new(total: usize) -> Self {
        Self {
            current_step: 0,
            total_steps: total,
            current_label: String::new(),
            percent: 0,
            log: Vec::new(),
        }
    }

    pub fn update(&mut self, step: usize, total: usize, label: &str) {
        self.current_step = step;
        self.total_steps = total;
        self.current_label = label.to_string();
        self.percent = if total == 0 { 100 } else { ((step as f64 / total as f64) * 100.0) as u8 };
        if !label.trim().is_empty() {
            self.log.push(label.to_string());
        }
    }
}

// ── License state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LicenseState {
    pub accepted: bool,
}

// ── User info state ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserInfoState {
    pub name: String,
    pub organization: String,
    pub serial_key: String,
}

// ── Main state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerState {
    // Navigation
    pub pages: Vec<Page>,
    pub current_page_index: usize,
    #[serde(skip)]
    page_definitions: Vec<PageDefinition>,
    pub current_page_definition: Option<PageDefinition>,

    // App info (subset forwarded to UI)
    pub app_name: String,
    pub app_version: String,
    pub app_publisher: String,
    pub app_description: Option<String>,
    pub app_website: Option<String>,
    pub license_text: Option<String>,

    // Assets (base64-encoded for WebView transfer)
    pub logo_b64: Option<String>,
    pub banner_b64: Option<String>,

    // Theme
    pub accent_color: String,
    pub accent_dark_color: String,
    pub accent_light_color: String,
    pub background_color: String,
    pub surface_color: String,
    pub text_color: String,
    pub text_muted_color: String,
    pub border_color: String,
    pub success_color: String,
    pub success_bg_color: String,
    pub error_color: String,
    pub error_bg_color: String,
    pub progress_color: String,
    pub progress_light_color: String,
    pub font_family: String,
    pub border_radius: u8,
    pub theme_preset: String,
    pub window_width: u32,
    pub window_height: u32,

    // User choices
    pub install_dir: String,
    pub components: Vec<crate::parser::schema::Component>,
    pub selected_components: HashSet<String>,
    pub license: LicenseState,
    pub user_info: UserInfoState,
    pub custom_values: HashMap<String, String>,

    // Requirements
    pub requirement_results: Vec<CheckResult>,
    pub requirements_passed: bool,

    // Install progress
    pub progress: Option<InstallProgress>,
    pub install_succeeded: Option<bool>,
    pub install_error: Option<String>,

    // Finish options
    pub launch_app: bool,
    pub create_desktop_shortcut: bool,

    // Silent mode
    pub silent: bool,

    // Operation mode
    pub is_uninstall: bool,
}

impl InstallerState {
    pub fn from_manifest(manifest: &InstallerManifest) -> Self {
        let page_definitions = manifest.pages.clone();
        let pages: Vec<Page> = page_definitions
            .iter()
            .enumerate()
            .map(|(index, page)| page_to_runtime_page(page, index))
            .collect();

        let install_dir = manifest.app.default_install_dir.clone()
            .unwrap_or_else(|| {
                let pf = std::env::var("ProgramFiles")
                    .unwrap_or_else(|_| "C:\\Program Files".into());
                format!("{}\\{}", pf, manifest.app.name)
            });
        let declared_vars = manifest.variables.as_ref()
            .map(|v| v.resolve_for_os(std::env::consts::OS))
            .unwrap_or_default();
        let install_dir = resolve_manifest_vars(&install_dir, &declared_vars);

        // Default all non-required components as selected
        let selected_components: HashSet<String> = manifest.components
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|c| c.selected || c.required)
            .map(|c| c.id.clone())
            .collect();

        let theme = manifest.theme.as_ref();
        let license_text = manifest
            .pages
            .iter()
            .find(|p| p.page_type == PageType::License)
            .and_then(|p| p.data.as_ref())
            .and_then(|d| d.get("text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let custom_values = collect_custom_defaults(&page_definitions);

        let current_page_definition = page_definitions.first().cloned();

        Self {
            pages,
            current_page_index: 0,
            page_definitions,
            current_page_definition,

            app_name:        manifest.app.name.clone(),
            app_version:     manifest.app.version.clone(),
            app_publisher:   manifest.app.publisher.clone(),
            app_description: manifest.app.description.clone(),
            app_website:     manifest.app.website.clone(),
            license_text,

            logo_b64:   None, // loaded by runner from embedded assets
            banner_b64: None,

            accent_color:      theme.and_then(|t| t.accent_color.clone()).unwrap_or("#0078D4".into()),
            accent_dark_color: theme.and_then(|t| t.accent_dark_color.clone()).unwrap_or("#005A9E".into()),
            accent_light_color: theme.and_then(|t| t.accent_light_color.clone()).unwrap_or("#EBF3FB".into()),
            background_color:  theme.and_then(|t| t.background_color.clone()).unwrap_or("#FFFFFF".into()),
            surface_color:     theme.and_then(|t| t.surface_color.clone()).unwrap_or("#F5F5F5".into()),
            text_color:        theme.and_then(|t| t.text_color.clone()).unwrap_or("#1A1A1A".into()),
            text_muted_color:  theme.and_then(|t| t.text_muted_color.clone()).unwrap_or("#6B6B6B".into()),
            border_color:      theme.and_then(|t| t.border_color.clone()).unwrap_or("#E0E0E0".into()),
            success_color:     theme.and_then(|t| t.success_color.clone()).unwrap_or("#107C10".into()),
            success_bg_color:  theme.and_then(|t| t.success_bg_color.clone()).unwrap_or("#F7F9F8".into()),
            error_color:       theme.and_then(|t| t.error_color.clone()).unwrap_or("#C42B1C".into()),
            error_bg_color:    theme.and_then(|t| t.error_bg_color.clone()).unwrap_or("#FFF7F6".into()),
            progress_color:    theme.and_then(|t| t.progress_color.clone()).unwrap_or("#0078D4".into()),
            progress_light_color: theme.and_then(|t| t.progress_light_color.clone()).unwrap_or("#EBF3FB".into()),
            font_family:       theme.and_then(|t| t.font_family.clone()).unwrap_or("Segoe UI, sans-serif".into()),
            border_radius:     theme.and_then(|t| t.border_radius).unwrap_or(6),
            theme_preset:      theme.and_then(|t| t.preset.clone()).unwrap_or("default".into()),
            window_width:      theme.and_then(|t| t.window_width).unwrap_or(780),
            window_height:     theme.and_then(|t| t.window_height).unwrap_or(540),

            install_dir,
            components: manifest.components.clone().unwrap_or_default(),
            selected_components,
            license:   LicenseState::default(),
            user_info: UserInfoState::default(),
            custom_values,

            requirement_results: Vec::new(),
            requirements_passed: true,

            progress:           None,
            install_succeeded:  None,
            install_error:      None,

            launch_app:              true,
            create_desktop_shortcut: true,
            silent: false,
            is_uninstall: false,
        }
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    pub fn current_page(&self) -> &Page {
        &self.pages[self.current_page_index]
    }

    pub fn can_go_next(&self) -> bool {
        // Can't go next from Install or Finish pages
        match self.current_page() {
            Page::Install => self.install_succeeded == Some(true),
            Page::Finish | Page::Error => false,
            Page::License => self.license.accepted,
            Page::Requirements => self.requirements_passed,
            Page::Custom(_) => self.custom_page_is_valid(),
            _ => true,
        }
    }

    pub fn go_next(&mut self) -> bool {
        if self.current_page_index + 1 < self.pages.len() {
            self.current_page_index += 1;
            self.sync_current_page_definition();
            true
        } else {
            false
        }
    }

    pub fn go_back(&mut self) -> bool {
        if self.current_page_index > 0 {
            self.current_page_index -= 1;
            self.sync_current_page_definition();
            true
        } else {
            false
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.current_page_index > 0
            && !matches!(self.current_page(), Page::Install | Page::Finish | Page::Error)
    }

    pub fn navigate_to(&mut self, page: Page) {
        if let Some(idx) = self.pages.iter().position(|p| p == &page) {
            self.current_page_index = idx;
            self.sync_current_page_definition();
        }
    }

    // ── Serialization for IPC ─────────────────────────────────────────────────

    /// Serialize just the fields the UI needs for the current page.
    pub fn to_ui_json(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(ref mut obj) = v {
            obj.insert("can_go_back_state".into(), serde_json::Value::Bool(self.can_go_back()));
            obj.insert("can_go_next_state".into(), serde_json::Value::Bool(self.can_go_next()));
        }
        v
    }

    pub fn set_custom_value(&mut self, key: &str, value: String) {
        self.custom_values.insert(key.to_string(), value);
    }

    fn sync_current_page_definition(&mut self) {
        self.current_page_definition = self.page_definitions.get(self.current_page_index).cloned();
    }

    fn custom_page_is_valid(&self) -> bool {
        let Some(page) = &self.current_page_definition else {
            return true;
        };

        if !matches!(page.page_type, PageType::Custom) {
            return true;
        }

        let Some(widgets) = page.widgets.as_ref() else {
            return true;
        };

        for widget in widgets {
            match widget {
                CustomWidget::Label(_) => {}
                CustomWidget::TextInput(spec) | CustomWidget::MultilineInput(spec) => {
                    let Some(bind_to) = spec.bind_to.as_ref() else { return false; };
                    let value = self.custom_values.get(bind_to).map(|v| v.trim()).unwrap_or("");
                    if spec.required && value.is_empty() {
                        return false;
                    }
                }
                CustomWidget::Checkbox(spec) => {
                    if spec.bind_to.is_none() {
                        return false;
                    }
                }
                CustomWidget::RadioGroup(spec) | CustomWidget::Dropdown(spec) => {
                    let Some(bind_to) = spec.bind_to.as_ref() else { return false; };
                    let value = self.custom_values.get(bind_to).map(|v| v.trim()).unwrap_or("");
                    if spec.required && value.is_empty() {
                        return false;
                    }
                    if !value.is_empty() && !spec.options.iter().any(|opt| opt.value == value) {
                        return false;
                    }
                }
                CustomWidget::FolderPicker(spec) => {
                    let Some(bind_to) = spec.bind_to.as_ref() else { return false; };
                    let value = self.custom_values.get(bind_to).map(|v| v.trim()).unwrap_or("");
                    if spec.required && value.is_empty() {
                        return false;
                    }
                    if spec.must_exist && !value.is_empty() && !std::path::Path::new(value).exists() {
                        return false;
                    }
                }
            }
        }

        true
    }
}

fn page_to_runtime_page(page: &PageDefinition, index: usize) -> Page {
    match page.page_type {
        PageType::Welcome => Page::Welcome,
        PageType::License => Page::License,
        PageType::Requirements => Page::Requirements,
        PageType::InstallDir => Page::InstallDir,
        PageType::Components => Page::Components,
        PageType::UserInfo => Page::UserInfo,
        PageType::Summary => Page::Summary,
        PageType::Install => Page::Install,
        PageType::Finish => Page::Finish,
        PageType::Error => Page::Error,
        PageType::Custom => Page::Custom(
            page.id
                .clone()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| format!("custom-{}", index + 1)),
        ),
    }
}

fn collect_custom_defaults(pages: &[PageDefinition]) -> HashMap<String, String> {
    let mut values = HashMap::new();

    for page in pages {
        let Some(widgets) = page.widgets.as_ref() else {
            continue;
        };

        for widget in widgets {
            match widget {
                CustomWidget::Label(_) => {}
                CustomWidget::TextInput(spec) | CustomWidget::MultilineInput(spec) => {
                    if let Some(bind_to) = spec.bind_to.as_ref() {
                        if let Some(default) = spec.default.as_ref() {
                            values.entry(bind_to.clone()).or_insert_with(|| default.clone());
                        }
                    }
                }
                CustomWidget::Checkbox(spec) => {
                    if let Some(bind_to) = spec.bind_to.as_ref() {
                        values.entry(bind_to.clone()).or_insert_with(|| spec.default.to_string());
                    }
                }
                CustomWidget::RadioGroup(spec) | CustomWidget::Dropdown(spec) => {
                    if let Some(bind_to) = spec.bind_to.as_ref() {
                        let default = spec.default.clone().or_else(|| spec.options.first().map(|opt| opt.value.clone()));
                        if let Some(default) = default {
                            values.entry(bind_to.clone()).or_insert(default);
                        }
                    }
                }
                CustomWidget::FolderPicker(spec) => {
                    if let Some(bind_to) = spec.bind_to.as_ref() {
                        if let Some(default) = spec.default.as_ref() {
                            values.entry(bind_to.clone()).or_insert_with(|| default.clone());
                        }
                    }
                }
            }
        }
    }

    values
}

fn resolve_manifest_vars(input: &str, declared_vars: &HashMap<String, String>) -> String {
    let mut s = input.to_string();

    for _ in 0..10 {
        let before = s.clone();
        for (key, value) in declared_vars {
            let normalized = key.trim().trim_start_matches('$');
            if normalized.is_empty() {
                continue;
            }
            let token_dollar = format!("${}", normalized);
            let token_template = format!("{{{{{}}}}}", normalized);
            s = s.replace(&token_dollar, value);
            s = s.replace(&token_template, value);
        }
        if s == before {
            break;
        }
    }

    let pf64 = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or("C:\\Program Files".into());
    s = s.replace("$PROGRAMFILES64", &pf64);
    s = s.replace("{{PROGRAMFILES64}}", &pf64);

    let pf = std::env::var("ProgramFiles").unwrap_or("C:\\Program Files".into());
    s = s.replace("$PROGRAMFILES", &pf);
    s = s.replace("{{PROGRAMFILES}}", &pf);

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    s = s.replace("$APPDATA", &appdata);
    s = s.replace("{{APPDATA}}", &appdata);

    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    s = s.replace("$LOCALAPPDATA", &local);
    s = s.replace("{{LOCALAPPDATA}}", &local);

    #[cfg(windows)]
    {
        s = s.replace('/', "\\");
    }
    s
}