#![windows_subsystem = "windows"]

mod window;

use anyhow::{Context, Result};
use engine::{
    install::{InstallContext, StepRunner},
    ipc::{parse_inbound, InboundMessage, OutboundEvent},
    parser,
    requirements,
    state::{InstallProgress, InstallerState, Page},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

include!("../../hagane/generated/embedded.rs");

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    if let Err(e) = run() {
        log::error!("Fatal: {:#}", e);
        #[cfg(windows)]
        show_error_dialog("Installer Error", &format!("{:#}", e));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let manifest_yaml = std::str::from_utf8(MANIFEST_YAML)
        .context("Manifest is not valid UTF-8")?;
    let manifest = parser::load_from_str(manifest_yaml)
        .context("Failed to parse installer manifest")?;

    let mut state = InstallerState::from_manifest(&manifest);

    let args: Vec<String> = std::env::args().collect();

    let is_silent = args.iter().any(|a| a == "/S" || a == "--silent" || a == "-s");

    #[cfg(windows)]
    {
        let exe_is_uninstaller = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|n| n.eq_ignore_ascii_case("uninstall.exe"))
            .unwrap_or(false);

        let arg_uninstall = args.iter().any(|a| {
            a.eq_ignore_ascii_case("/UNINSTALL")
                || a.eq_ignore_ascii_case("--uninstall")
                || a.eq_ignore_ascii_case("-u")
        });

        if exe_is_uninstaller || arg_uninstall {
            if is_silent {
                return run_uninstall(manifest, state);
            }
            prepare_uninstall_state(&mut state)?;
        }
    }

    if is_silent {
        state.silent = true;
        return run_silent(manifest, state);
    }

    if !ASSET_LOGO.is_empty() {
        state.logo_b64 = Some(data_url_from_bytes(ASSET_LOGO));
    } else if !ASSET_ICON.is_empty() {
        state.logo_b64 = Some(data_url_from_bytes(ASSET_ICON));
    }
    if !ASSET_BANNER.is_empty() { state.banner_b64 = Some(data_url_from_bytes(ASSET_BANNER)); }

    let state    = Arc::new(Mutex::new(state));
    let manifest = Arc::new(manifest);
    let archives: HashMap<String, Vec<u8>> = if ARCHIVE_MAP.is_empty() {
        HashMap::new()
    } else {
        serde_json::from_slice(ARCHIVE_MAP).unwrap_or_default()
    };
    let archives = Arc::new(archives);

    use windows::core::PCWSTR;

    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use webview2_com::{
            CreateCoreWebView2EnvironmentCompletedHandler,
            CreateCoreWebView2ControllerCompletedHandler,
            WebMessageReceivedEventHandler,
        };
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok(); }

        let (win_w, win_h) = {
            let st = state.lock().unwrap();
            (st.window_width, st.window_height)
        };
        let title = {
            let st = state.lock().unwrap();
            if st.is_uninstall {
                format!("{} — Uninstall", st.app_name)
            } else {
                format!("{} — Setup", st.app_name)
            }
        };
        let hwnd = window::create_window(&title, win_w, win_h)?;

        let user_data = std::env::temp_dir().join("installer_webview2_data");
        let user_data_str = user_data.to_string_lossy().to_string();

        let html_map = Arc::new(build_html_map());

        let state_cb    = Arc::clone(&state);
        let manifest_cb = Arc::clone(&manifest);
        let archives_cb = Arc::clone(&archives);
        let html_map_cb = Arc::clone(&html_map);

        // Shared controller handle for resize etc.
        let ctrl_holder: Arc<Mutex<Option<ICoreWebView2Controller>>> = Arc::new(Mutex::new(None));
        let ctrl_cb = Arc::clone(&ctrl_holder);

        let user_data_wide: Vec<u16> = user_data_str.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                PCWSTR(user_data_wide.as_ptr()),
                None,
                &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
                    move |_hr, env| {
                        let env = match env { Some(e) => e, None => return Ok(()) };

                        let state_i    = Arc::clone(&state_cb);
                        let manifest_i = Arc::clone(&manifest_cb);
                        let archives_i = Arc::clone(&archives_cb);
                        let html_i     = Arc::clone(&html_map_cb);
                        let ctrl_i     = Arc::clone(&ctrl_cb);

                        env.CreateCoreWebView2Controller(
                            hwnd,
                            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                                move |_hr, ctrl| {
                                    let ctrl: ICoreWebView2Controller = match ctrl {
                                        Some(c) => c,
                                        None => return Ok(()),
                                    };

                                    use windows::Win32::Foundation::RECT;
                                    let mut bounds = RECT::default();
                                    windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut bounds).ok();
                                    if (bounds.right - bounds.left) <= 0 || (bounds.bottom - bounds.top) <= 0 {
                                        // Hidden windows can report a zero client rect before first show.
                                        // Seed WebView with the configured window size to avoid blank white host.
                                        bounds.right = win_w as i32;
                                        bounds.bottom = win_h as i32;
                                    }
                                    ctrl.SetBounds(bounds).ok();

                                    let webview: ICoreWebView2 = ctrl.CoreWebView2()?;

                                    if let Ok(settings) = webview.Settings() {
                                        settings.SetAreDefaultContextMenusEnabled(false).ok();
                                        settings.SetAreDevToolsEnabled(cfg!(debug_assertions)).ok();
                                        settings.SetIsStatusBarEnabled(false).ok();
                                    }

                                    let state_msg    = Arc::clone(&state_i);
                                    let manifest_msg = Arc::clone(&manifest_i);
                                    let archives_msg = Arc::clone(&archives_i);
                                    let html_msg     = Arc::clone(&html_i);
                                    let wv_msg       = webview.clone();

                                    use windows::core::PWSTR;
                                    let mut token = Default::default();

                                    webview.add_WebMessageReceived(
                                        &WebMessageReceivedEventHandler::create(Box::new(
                                            move |_wv, args| {
                                                if let Some(args) = args {
                                                    let mut raw_pwstr = PWSTR::null();
                                                    if args.TryGetWebMessageAsString(&mut raw_pwstr).is_ok() {
                                                        let msg = raw_pwstr.to_string().unwrap_or_default();
                                                        handle_message(msg, &wv_msg, &state_msg, &manifest_msg, &archives_msg, &html_msg);

                                                    }
                                                }
                                                Ok(())
                                            }
                                        )),
                                        &mut token,
                                    ).ok();

                                    // Load first page
                                    if let Some(html) = html_i.get("shell") {
                                        let html_wide: Vec<u16> = html.encode_utf16().chain(std::iter::once(0)).collect();
                                        webview.NavigateToString(windows::core::PCWSTR(html_wide.as_ptr())).ok();
                                    }

                                    *ctrl_i.lock().unwrap() = Some(ctrl);
                                    Ok(())
                                }
                            ))
                        ).ok();
                        Ok(())
                    }
                )),
            ).context("CreateCoreWebView2EnvironmentWithOptions failed")?;
        }

        window::run_message_loop();
    }

    #[cfg(not(windows))]
    log::warn!("GUI mode only supported on Windows. Use --silent for headless.");

    Ok(())
}

// ── Message handler ───────────────────────────────────────────────────────────

#[cfg(windows)]
fn handle_message(
    raw: String,
    webview: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
    state: &Arc<Mutex<InstallerState>>,
    manifest: &Arc<engine::parser::schema::InstallerManifest>,
    archives: &Arc<HashMap<String, Vec<u8>>>,
    html_map: &Arc<HashMap<String, String>>,
) {
    let msg = match parse_inbound(&raw) {
        Ok(m) => m,
        Err(e) => { log::error!("IPC parse error: {} | raw: {}", e, raw); return; }
    };

    let mut st = state.lock().unwrap();

    match msg {
        InboundMessage::Ready => {
            send_state(webview, &st, true);
            let page_name = page_to_filename(st.current_page()).to_string();
            if let Some(html) = html_map.get(&page_name) {
                send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
            }
        }

        InboundMessage::GetState => {
            send_state(webview, &st, false);
        }

        InboundMessage::Next => {
            // On Install page, Next is used by progress.html to trigger the actual install.
            if matches!(st.current_page(), engine::state::Page::Install) {
                if st.install_succeeded.is_none() {
                    let total = if st.is_uninstall { 4 } else { manifest.steps.len() };
                    st.progress = Some(InstallProgress::new(total));
                    st.install_error = None;

                    let install_dir = st.install_dir.clone();
                    let selected_components = st.selected_components.clone();
                    let is_uninstall = st.is_uninstall;
                    let state_clone = Arc::clone(state);
                    let manifest_clone = Arc::clone(manifest);
                    let archives_clone = Arc::clone(archives);
                    let initial_state = to_state_json(&st, false);
                    drop(st);

                    std::thread::spawn(move || {
                        let result = if is_uninstall {
                            run_uninstall_tasks(&manifest_clone, PathBuf::from(&install_dir), |step, total, label| {
                                let mut s = state_clone.lock().unwrap();
                                if s.progress.is_none() {
                                    s.progress = Some(InstallProgress::new(total));
                                }
                                if let Some(p) = s.progress.as_mut() {
                                    p.update(step, total, label);
                                }
                            })
                        } else {
                            let ctx = InstallContext {
                                install_dir: PathBuf::from(&install_dir),
                                selected_components,
                                archives: (*archives_clone).clone(),
                                backup_dir: std::env::temp_dir().join("installer_backup"),
                                logging: manifest_clone.logging.clone(),
                            };

                            let mut runner = StepRunner::new(ctx);
                            runner.run_all(&manifest_clone.steps, |step, total, label| {
                                let mut s = state_clone.lock().unwrap();
                                if s.progress.is_none() {
                                    s.progress = Some(InstallProgress::new(total));
                                }
                                if let Some(p) = s.progress.as_mut() {
                                    p.update(step + 1, total, label);
                                }
                            })
                        };

                        let mut s = state_clone.lock().unwrap();
                        match result {
                            Ok(()) => {
                                s.install_succeeded = Some(true);
                                s.install_error = None;
                                let is_uninstall = s.is_uninstall;
                                if let Some(p) = s.progress.as_mut() {
                                    p.percent = 100;
                                    p.current_label = if is_uninstall {
                                        "Uninstall complete".to_string()
                                    } else {
                                        "Done".to_string()
                                    };
                                }
                            }
                            Err(e) => {
                                s.install_succeeded = Some(false);
                                s.install_error = Some(format!("{:#}", e));
                                if let Some(p) = s.progress.as_mut() {
                                    p.current_label = "Failed".to_string();
                                }
                            }
                        }
                    });

                    send_event(webview, &OutboundEvent::StateUpdate { state: initial_state });
                    return;
                }
            }

            // On terminal pages, Next should close the installer.
            if matches!(st.current_page(), engine::state::Page::Finish | engine::state::Page::Error) {
                drop(st);
                unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0); }
                return;
            }

            if st.can_go_next() {
                st.go_next();
                let next_page = st.current_page().clone();

                // Kick off parallel requirement checks when entering that page
                if next_page == engine::state::Page::Requirements {
                    if let Some(reqs) = &manifest.requirements {
                        let reqs = reqs.clone();
                        let install_dir = st.install_dir.clone();
                        let state_clone = Arc::clone(state);
                        std::thread::spawn(move || {
                            let results = requirements::run_all(&reqs, &install_dir);
                            let all_passed = results.iter().all(|r| r.passed);
                            {
                                let mut s = state_clone.lock().unwrap();
                                s.requirement_results = results;
                                s.requirements_passed = all_passed;
                            }
                        });

                        drop(st);
                        let page_name = page_to_filename(&next_page).to_string();
                        if let Some(html) = html_map.get(&page_name) {
                            send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                        }
                        let st_now = state.lock().unwrap();
                        send_state(webview, &st_now, false);
                        return;
                    }
                }

                let page_name = page_to_filename(&next_page).to_string();
                if let Some(html) = html_map.get(&page_name) {
                    send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                }
                send_state(webview, &st, false);
            }
        }

        InboundMessage::Back => {
            if st.can_go_back() {
                st.go_back();
                let page_name = page_to_filename(st.current_page()).to_string();
                if let Some(html) = html_map.get(&page_name) {
                    send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                }
                send_state(webview, &st, false);
            }
        }

        InboundMessage::Cancel => {
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0); }
        }

        InboundMessage::LicenseAccepted { accepted } => {
            st.license.accepted = accepted;
            send_state(webview, &st, false);
        }

        InboundMessage::SetInstallDir { path } => {
            st.install_dir = path;
            send_state(webview, &st, false);
        }

        InboundMessage::SetComponent { id, selected } => {
            if selected { st.selected_components.insert(id); }
            else        { st.selected_components.remove(&id); }
            send_state(webview, &st, false);
        }

        InboundMessage::SetUserInfo { name, organization, serial_key } => {
            st.user_info.name = name;
            st.user_info.organization = organization;
            st.user_info.serial_key = serial_key;
        }

        InboundMessage::SetFinishOptions { launch_app, create_desktop_shortcut } => {
            st.launch_app = launch_app;
            st.create_desktop_shortcut = create_desktop_shortcut;
        }

        InboundMessage::OpenUrl { url } => {
            drop(st);
            open_external_url(&url);
            return;
        }

        InboundMessage::BrowseInstallDir => {
            drop(st);
            let path = browse_for_folder();
            send_event(webview, &OutboundEvent::BrowseResult { path });
            return;
        }

    }
}

// ── Silent install ────────────────────────────────────────────────────────────

fn run_silent(
    manifest: engine::parser::schema::InstallerManifest,
    state: InstallerState,
) -> Result<()> {
    log::info!("Silent install — {}", state.app_name);

    if let Some(reqs) = &manifest.requirements {
        let results = requirements::run_all(reqs, &state.install_dir);
        let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
        if !failed.is_empty() {
            for f in &failed { log::error!("Requirement FAILED: {} — {}", f.label, f.detail); }
            anyhow::bail!("System requirements not met");
        }
    }

    let archives: HashMap<String, Vec<u8>> = if ARCHIVE_MAP.is_empty() {
        HashMap::new()
    } else {
        serde_json::from_slice(ARCHIVE_MAP).unwrap_or_default()
    };

    let ctx = InstallContext {
        install_dir: PathBuf::from(&state.install_dir),
        selected_components: state.selected_components,
        archives,
        backup_dir: std::env::temp_dir().join("installer_backup"),
        logging: manifest.logging.clone(),
    };

    let mut runner = StepRunner::new(ctx);
    runner.run_all(&manifest.steps, |step, total, label| {
        log::info!("[{}/{}] {}", step + 1, total, label);
    })?;

    log::info!("Installation complete.");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn page_to_filename(page: &engine::state::Page) -> &'static str {
    use engine::state::Page;
    match page {
        Page::Welcome      => "welcome",
        Page::License      => "license",
        Page::Requirements => "requirements",
        Page::InstallDir   => "install_dir",
        Page::Components   => "components",
        Page::UserInfo     => "user_info",
        Page::Summary      => "summary",
        Page::Install      => "progress",
        Page::Finish       => "finish",
        Page::Error        => "error",
    }
}

fn build_html_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("shell".into(),        include_str!("../../ui/pages/shell.html").into());
    map.insert("welcome".into(),      include_str!("../../ui/pages/welcome.html").into());
    map.insert("license".into(),      include_str!("../../ui/pages/license.html").into());
    map.insert("requirements".into(), include_str!("../../ui/pages/requirements.html").into());
    map.insert("install_dir".into(),  include_str!("../../ui/pages/install_dir.html").into());
    map.insert("components".into(),   include_str!("../../ui/pages/components.html").into());
    map.insert("user_info".into(),    include_str!("../../ui/pages/user_info.html").into());
    map.insert("summary".into(),      include_str!("../../ui/pages/summary.html").into());
    map.insert("progress".into(),     include_str!("../../ui/pages/progress.html").into());
    map.insert("finish".into(),       include_str!("../../ui/pages/finish.html").into());
    map.insert("error".into(),        include_str!("../../ui/pages/error.html").into());
    map
}

#[cfg(windows)]
fn send_event(
    webview: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
    event: &OutboundEvent,
) {
    let js = event.to_js_call();
    let js_wide: Vec<u16> = js.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { webview.ExecuteScript(windows::core::PCWSTR(js_wide.as_ptr()), None).ok(); }
}

#[cfg(windows)]
fn to_state_json(st: &InstallerState, include_assets: bool) -> serde_json::Value {
    let mut json = st.to_ui_json();
    if !include_assets {
        if let serde_json::Value::Object(ref mut obj) = json {
            obj.insert("logo_b64".into(), serde_json::Value::Null);
            obj.insert("banner_b64".into(), serde_json::Value::Null);
        }
    }
    json
}

#[cfg(windows)]
fn send_state(
    webview: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
    st: &InstallerState,
    include_assets: bool,
) {
    send_event(
        webview,
        &OutboundEvent::StateUpdate {
            state: to_state_json(st, include_assets),
        },
    );
}

#[cfg(windows)]
fn browse_for_folder() -> Option<String> {
    use windows::Win32::UI::Shell::{
        SHBrowseForFolderW, SHGetPathFromIDListW, BROWSEINFOW,
        BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS,
    };
    unsafe {
        let title: Vec<u16> = "Select installation folder\0"
            .encode_utf16().collect();
        let mut bi = BROWSEINFOW {
            lpszTitle: windows::core::PCWSTR(title.as_ptr()),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            ..Default::default()
        };
        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() { return None; }
        let mut path = [0u16; 260];
        if SHGetPathFromIDListW(pidl, &mut path).as_bool() {
            let len = path.iter().position(|&c| c == 0).unwrap_or(0);
            Some(String::from_utf16_lossy(&path[..len]).to_string())
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn show_error_dialog(title: &str, message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let t: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let m: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(None,
            windows::core::PCWSTR(m.as_ptr()),
            windows::core::PCWSTR(t.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(windows)]
fn prepare_uninstall_state(state: &mut InstallerState) -> Result<()> {
    let current_exe = std::env::current_exe().context("Unable to locate uninstaller executable")?;
    let install_dir = current_exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(&state.install_dir));

    state.install_dir = install_dir.to_string_lossy().to_string();
    state.is_uninstall = true;
    state.pages = vec![Page::Welcome, Page::Summary, Page::Install, Page::Finish];
    state.current_page_index = 0;
    state.install_succeeded = None;
    state.install_error = None;
    state.progress = None;
    state.app_description = Some(format!("This will remove {} from your computer.", state.app_name));
    Ok(())
}

#[cfg(windows)]
fn run_uninstall_tasks<F>(
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: PathBuf,
    mut progress: F,
) -> Result<()>
where
    F: FnMut(usize, usize, &str),
{
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::process::Command;

    let current_exe = std::env::current_exe().context("Unable to locate uninstaller executable")?;
    log::info!("Uninstall mode — {}", manifest.app.name);
    log::info!("Resolved install directory: {}", install_dir.display());

    let has_extra_steps = manifest
        .uninstall
        .as_ref()
        .and_then(|u| u.extra_steps.as_ref())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let total_steps = if has_extra_steps { 6 } else { 5 };
    let mut step_no = 1usize;

    progress(step_no, total_steps, "Removing uninstall registry entries");
    step_no += 1;

    // Best-effort cleanup of known registry locations defined by manifest.
    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::Registry(r) = step {
            if matches!(r.operation, engine::parser::schema::RegistryOperation::Write)
                && r.key.contains("CurrentVersion\\Uninstall\\")
            {
                let _ = Command::new("reg")
                    .args(["delete", &format!("{}\\{}", r.hive, r.key), "/f"])
                    .status();
            }
        }
    }

    progress(step_no, total_steps, "Removing application registry keys");
    step_no += 1;

    if let Some(app_key) = &manifest.app.registry_key {
        let full_key = format!("SOFTWARE\\{}", app_key);
        let _ = Command::new("reg")
            .args(["delete", &format!("HKLM\\{}", full_key), "/f"])
            .status();
        let _ = Command::new("reg")
            .args(["delete", &format!("HKCU\\{}", full_key), "/f"])
            .status();
    }

    if let Some(extra_steps) = manifest.uninstall.as_ref().and_then(|u| u.extra_steps.as_ref()) {
        if !extra_steps.is_empty() {
            progress(step_no, total_steps, "Running uninstall extra steps");
            step_no += 1;
            let ctx = InstallContext {
                install_dir: install_dir.clone(),
                selected_components: HashSet::new(),
                archives: HashMap::new(),
                backup_dir: std::env::temp_dir().join("uninstall_backup"),
                logging: manifest.logging.clone(),
            };
            let mut runner = StepRunner::new(ctx);
            runner.run_all(extra_steps, |_step, _total, _label| {})?;
        }
    }

    progress(step_no, total_steps, "Removing installed files");
    step_no += 1;

    remove_install_contents(&install_dir, &current_exe)?;

    progress(step_no, total_steps, "Scheduling self-delete and final cleanup");
    step_no += 1;

    // Schedule self-delete and final directory cleanup after process exits.
    // Retries are important because uninstall.exe is still locked until this process terminates.
    let exe_escaped = current_exe.to_string_lossy().replace('"', "\"\"").replace('\'', "''");
    let dir_escaped = install_dir.to_string_lossy().replace('"', "\"\"").replace('\'', "''");
    let parent_escaped = install_dir
        .parent()
        .map(|p| p.to_string_lossy().replace('"', "\"\"").replace('\'', "''"))
        .unwrap_or_default();
    let ps_script = format!(
        "$exe='{}'; $dir='{}'; $parent='{}'; Set-Location -LiteralPath $env:TEMP; for($i=0;$i -lt 120;$i++){{ Remove-Item -LiteralPath $exe -Force -ErrorAction SilentlyContinue; if(-not (Test-Path -LiteralPath $exe)){{ break }}; Start-Sleep -Milliseconds 500 }}; Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue; if($parent -and (Test-Path -LiteralPath $parent)){{ $count=(Get-ChildItem -LiteralPath $parent -Force -ErrorAction SilentlyContinue | Measure-Object).Count; if($count -eq 0){{ Remove-Item -LiteralPath $parent -Force -ErrorAction SilentlyContinue }} }}",
        exe_escaped,
        dir_escaped,
        parent_escaped
    );
    Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .current_dir(std::env::temp_dir())
        .spawn()
        .context("Failed to schedule uninstall cleanup")?;

    progress(step_no, total_steps, "Cleanup scheduled. Close to finish uninstall");
    log::info!("Uninstall scheduled. Exiting.");
    Ok(())
}

#[cfg(windows)]
fn remove_install_contents(install_dir: &PathBuf, current_exe: &PathBuf) -> Result<()> {
    if !install_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(install_dir)
        .with_context(|| format!("Failed to list install dir: {}", install_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path == *current_exe {
            continue;
        }

        if path.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                log::warn!("Failed to remove directory '{}': {}", path.display(), e);
            }
        } else if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("Failed to remove file '{}': {}", path.display(), e);
        }
    }

    Ok(())
}

#[cfg(windows)]
fn run_uninstall(
    manifest: engine::parser::schema::InstallerManifest,
    mut state: InstallerState,
) -> Result<()> {
    prepare_uninstall_state(&mut state)?;
    run_uninstall_tasks(&manifest, PathBuf::from(&state.install_dir), |_step, _total, _label| {})
}

fn data_url_from_bytes(data: &[u8]) -> String {
    let mime = if data.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        "image/png"
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.starts_with(b"BM") {
        "image/bmp"
    } else if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        "image/x-icon"
    } else {
        "application/octet-stream"
    };

    format!("data:{};base64,{}", mime, base64_encode(data))
}

#[cfg(windows)]
fn open_external_url(url: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let op_w: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let url_w: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = ShellExecuteW(
            None,
            PCWSTR(op_w.as_ptr()),
            PCWSTR(url_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}