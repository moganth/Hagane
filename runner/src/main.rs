#![windows_subsystem = "windows"]

mod window;
#[cfg(not(windows))]
mod wry_window;

use anyhow::{Context, Result};
use engine::{
    install::{InstallContext, StepRunner},
    parser,
    requirements,
    state::InstallerState,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[cfg(windows)]
use engine::{
    ipc::{parse_inbound, InboundMessage, OutboundEvent},
    state::{InstallProgress, Page},
};
#[cfg(windows)]
use std::process::Command;

include!("../../hagane/generated/embedded.rs");
include!(concat!(env!("OUT_DIR"), "/theme_registry.rs"));

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

    // On non-Windows, WebView2 is unavailable — always run headless.
    // Respect --silent / custom flags on all platforms.
    let args: Vec<String> = std::env::args().collect();
    let is_silent = {
        let default_flags = vec!["/S".to_string(), "--silent".to_string(), "-s".to_string()];
        let silent_flags: &[String] = manifest.silent.as_ref()
            .and_then(|s| s.flags.as_ref())
            .map(|f| f.as_slice())
            .unwrap_or(&default_flags);
        args.iter().any(|a| silent_flags.contains(a))
    };

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

    // Linux: check for uninstall flag (runs uninstall.sh logic directly)
    #[cfg(not(windows))]
    {
        let arg_uninstall = args.iter().any(|a| {
            a.eq_ignore_ascii_case("--uninstall") || a.eq_ignore_ascii_case("-u")
        });
        if arg_uninstall {
            return run_uninstall_linux(manifest, state);
        }
    }

    // Linux: re-exec with sudo if require_admin is set and we are not root.
    #[cfg(not(windows))]
    if manifest.app.require_admin {
        let uid = unsafe { libc::getuid() };
        if uid != 0 {
            let exe = std::env::current_exe().context("cannot resolve own executable path")?;
            log::info!("Elevation required — re-launching with sudo");
            let status = std::process::Command::new("sudo")
                .arg(&exe)
                .args(&args[1..])
                .status()
                .context("Failed to launch sudo for elevation")?;
            std::process::exit(status.code().unwrap_or(1));
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

    // state/manifest/archives are only consumed by the GUI path.
    #[cfg(not(windows))]
    {
        let html_map = build_html_map();
        return wry_window::run_gui(
            Arc::try_unwrap(manifest).unwrap_or_else(|a| (*a).clone()),
            Arc::try_unwrap(state).map(|m| m.into_inner().unwrap()).unwrap_or_else(|a| a.lock().unwrap().clone()),
            Arc::try_unwrap(archives).unwrap_or_else(|a| (*a).clone()),
            html_map,
        );
    }

    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use webview2_com::{
            CreateCoreWebView2EnvironmentCompletedHandler,
            CreateCoreWebView2ControllerCompletedHandler,
            WebMessageReceivedEventHandler,
        };
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        use windows::core::PCWSTR;

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

    #[allow(unreachable_code)]
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
            if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
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
                    let custom_values = st.custom_values.clone();
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
                                variables: {
                                    let mut vars = manifest_clone.variables.as_ref()
                                        .map(|v| v.resolve_for_os(std::env::consts::OS))
                                        .unwrap_or_default();
                                    vars.extend(custom_values.clone());
                                    vars
                                },
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
                if matches!(st.current_page(), engine::state::Page::Finish)
                    && st.install_succeeded == Some(true)
                    && !st.is_uninstall
                {
                    let install_dir = PathBuf::from(&st.install_dir);
                    let selected_components = st.selected_components.clone();

                    if !st.create_desktop_shortcut {
                        if let Err(e) = remove_desktop_shortcuts_from_manifest(
                            manifest,
                            &install_dir,
                            &selected_components,
                        ) {
                            log::warn!("Failed to apply finish-page desktop shortcut option: {}", e);
                        }
                    }

                    if st.launch_app {
                        if let Some(launch) = build_launch_command(manifest, &install_dir, &selected_components) {
                            let mut cmd = Command::new(&launch.executable);
                            if let Some(args) = &launch.arguments {
                                if !args.trim().is_empty() {
                                    cmd.arg(args);
                                }
                            }
                            if let Some(work_dir) = &launch.working_dir {
                                if !work_dir.trim().is_empty() {
                                    cmd.current_dir(work_dir);
                                }
                            }
                            if let Err(e) = cmd.spawn() {
                                log::warn!("Failed to launch '{}' from finish page: {}", launch.executable, e);
                            }
                        } else {
                            log::warn!("Finish-page launch requested but no launchable executable was found.");
                        }
                    }
                }

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
                        let preset = {
                            let st_now = state.lock().unwrap();
                            st_now.theme_preset.clone()
                        };
                        if let Some(html) = select_page_html(html_map, &page_name, &preset) {
                            send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                        }
                        let st_now = state.lock().unwrap();
                        send_state(webview, &st_now, false);
                        return;
                    }
                }

                let page_name = page_to_filename(&next_page).to_string();
                if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
                    send_event(webview, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                }
                send_state(webview, &st, false);
            }
        }

        InboundMessage::Back => {
            if st.can_go_back() {
                st.go_back();
                let page_name = page_to_filename(st.current_page()).to_string();
                if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
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

        InboundMessage::SetCustomValue { id, value } => {
            st.set_custom_value(&id, value);
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
            let path = browse_for_folder(Some("Select installation folder"), None);
            send_event(webview, &OutboundEvent::BrowseResult { id: None, path });
            return;
        }

        InboundMessage::BrowseFolder { id, title, initial_path } => {
            drop(st);
            let path = browse_for_folder(title.as_deref(), initial_path.as_deref());
            send_event(webview, &OutboundEvent::BrowseResult { id: Some(id), path });
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
        variables: {
            let mut vars = manifest.variables.as_ref()
                .map(|v| v.resolve_for_os(std::env::consts::OS))
                .unwrap_or_default();
            vars.extend(state.custom_values);
            vars
        },
    };

    let mut runner = StepRunner::new(ctx);
    runner.run_all(&manifest.steps, |step, total, label| {
        // run_all calls with step == total for the final "Done" notification.
        let n = if step < total { step + 1 } else { total };
        log::info!("[{}/{}] {}", n, total, label);
    })?;

    log::info!("Installation complete.");
    Ok(())
}

// ── Linux uninstall ───────────────────────────────────────────────────────────

#[cfg(not(windows))]
fn run_uninstall_linux(
    manifest: engine::parser::schema::InstallerManifest,
    state: InstallerState,
) -> Result<()> {
    use std::io::{self, Write};

    let install_dir = PathBuf::from(&state.install_dir);
    log::info!("Uninstalling {} from {}", state.app_name, install_dir.display());

    // Confirm unless stdin is not a tty (piped).
    if atty_stdin() {
        print!("Remove {} from {}? [y/N] ", state.app_name, install_dir.display());
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Remove PATH entries from shell configs.
    let bin_dir = install_dir.join("bin").to_string_lossy().to_string();
    for rc in &[
        format!("/home/{}/.bashrc", std::env::var("SUDO_USER").unwrap_or_default()),
        format!("/home/{}/.profile", std::env::var("SUDO_USER").unwrap_or_default()),
        std::env::var("HOME").unwrap_or_default() + "/.bashrc",
        std::env::var("HOME").unwrap_or_default() + "/.profile",
    ] {
        if rc.trim_start_matches('/').is_empty() || !std::path::Path::new(rc).exists() { continue; }
        if let Ok(content) = std::fs::read_to_string(rc) {
            let cleaned: String = content
                .lines()
                .filter(|l| !l.contains("# hagane:") && !l.contains(&bin_dir))
                .map(|l| format!("{}\n", l))
                .collect();
            let _ = std::fs::write(rc, cleaned);
        }
    }

    // Remove system profile.d snippet.
    let app_name_slug = manifest.app.name.to_lowercase().replace(' ', "-");
    let _ = std::fs::remove_file(format!("/etc/profile.d/{}-path.sh", app_name_slug));

    // Run any custom uninstall extra_steps from the manifest.
    if let Some(extra) = manifest.uninstall.as_ref().and_then(|u| u.extra_steps.as_ref()) {
        if !extra.is_empty() {
            let ctx = InstallContext {
                install_dir: install_dir.clone(),
                selected_components: HashSet::new(),
                archives: HashMap::new(),
                backup_dir: std::env::temp_dir().join("uninstall_backup"),
                logging: manifest.logging.clone(),
                variables: manifest.variables.as_ref()
                    .map(|v| v.resolve_for_os(std::env::consts::OS))
                    .unwrap_or_default(),
            };
            let mut runner = StepRunner::new(ctx);
            let _ = runner.run_all(extra, |_, _, _| {});
        }
    }

    // Remove the install directory.
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .with_context(|| format!("Failed to remove {}", install_dir.display()))?;
        log::info!("Removed {}", install_dir.display());
    }

    println!("{} has been uninstalled.", state.app_name);
    Ok(())
}

#[cfg(not(windows))]
fn atty_stdin() -> bool {
    // Simple check: fd 0 is a tty.
    unsafe { libc::isatty(libc::STDIN_FILENO) != 0 }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[cfg(windows)]
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
        Page::Custom(_)    => "custom",
    }
}

#[cfg(windows)]
fn select_page_html<'a>(
    map: &'a HashMap<String, String>,
    page_name: &str,
    preset: &str,
) -> Option<&'a String> {
    if !preset.is_empty() {
        let themed_key = format!("{}__theme__{}", page_name, preset);
        if let Some(html) = map.get(&themed_key) {
            return Some(html);
        }
    }
    map.get(page_name)
}

fn render_theme_page_html(template: &str, progress_js: Option<&str>) -> String {
    if template.contains("__HAGANE_PROGRESS_JS__") {
        template.replace("__HAGANE_PROGRESS_JS__", progress_js.unwrap_or(""))
    } else {
        template.to_string()
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
    map.insert("custom".into(),       include_str!("../../ui/pages/custom.html").into());
    map.insert("finish".into(),       include_str!("../../ui/pages/finish.html").into());
    map.insert("error".into(),        include_str!("../../ui/pages/error.html").into());
    register_themed_html(&mut map);

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
    if let serde_json::Value::Object(ref mut obj) = json {
        let (global_css, page_css) = theme_css_bundle(&st.theme_preset);
        obj.insert("theme_global_css".into(), serde_json::Value::String(global_css.to_string()));
        obj.insert("theme_page_css".into(), serde_json::json!(page_css));
    }
    json
}

#[cfg(windows)]
fn theme_css_bundle(preset: &str) -> (&'static str, std::collections::HashMap<&'static str, &'static str>) {
    if let Some(bundle) = theme_css_bundle_generated(preset) {
        bundle
    } else {
        (include_str!("../../ui/themes/default/global.css"), std::collections::HashMap::new())
    }
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
fn browse_for_folder(title: Option<&str>, initial_path: Option<&str>) -> Option<String> {
    use windows::Win32::UI::Shell::{
        SHBrowseForFolderW, SHGetPathFromIDListW, BROWSEINFOW,
        BFFM_INITIALIZED, BFFM_SETSELECTIONW,
        BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS,
    };
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

    unsafe extern "system" fn browse_callback(hwnd: HWND, msg: u32, _lparam: LPARAM, data: LPARAM) -> i32 {
        if msg == BFFM_INITIALIZED && data.0 != 0 {
            SendMessageW(hwnd, BFFM_SETSELECTIONW, WPARAM(1), data);
        }
        0
    }

    unsafe {
        let default_title = title.unwrap_or("Select folder");
        let title: Vec<u16> = default_title.encode_utf16().chain(std::iter::once(0)).collect();
        let initial_path: Vec<u16> = initial_path
            .map(|path| path.encode_utf16().chain(std::iter::once(0)).collect())
            .unwrap_or_default();
        let mut bi = BROWSEINFOW {
            lpszTitle: windows::core::PCWSTR(title.as_ptr()),
            lpfn: Some(browse_callback),
            lParam: LPARAM(initial_path.as_ptr() as isize),
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

    let current_exe = std::env::current_exe().context("Unable to locate uninstaller executable")?;
    log::info!("Uninstall mode — {}", manifest.app.name);
    log::info!("Resolved install directory: {}", install_dir.display());

    let has_extra_steps = manifest
        .uninstall
        .as_ref()
        .and_then(|u| u.extra_steps.as_ref())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let total_steps = if has_extra_steps { 7 } else { 6 };
    let mut step_no = 1usize;

    progress(step_no, total_steps, "Removing uninstall registry entries");
    step_no += 1;

    // Best-effort cleanup of known registry locations defined by manifest.
    for step in &manifest.steps {
        match step {
            engine::parser::schema::InstallStep::Registry(r) => {
                let resolved_key = resolve_uninstall_var_string(&r.key, manifest, &install_dir).replace('/', "\\");
                if matches!(r.operation, engine::parser::schema::RegistryOperation::Write)
                    && resolved_key.contains("CurrentVersion\\Uninstall\\")
                {
                    let _ = Command::new("reg")
                        .args(["delete", &format!("{}\\{}", r.hive, resolved_key), "/f"])
                        .status();
                }
            }
            engine::parser::schema::InstallStep::RegisterUninstall(r) => {
                let resolved_key = resolve_uninstall_var_string(&r.key, manifest, &install_dir).replace('/', "\\");
                let _ = Command::new("reg")
                    .args(["delete", &format!("{}\\{}", r.hive, resolved_key), "/f"])
                    .status();
            }
            _ => {}
        }
    }

    progress(step_no, total_steps, "Removing application registry keys");
    step_no += 1;

    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::RegisterApp(r) = step {
            let resolved_key = resolve_uninstall_var_string(&r.key, manifest, &install_dir).replace('/', "\\");
            let _ = Command::new("reg")
                .args(["delete", &format!("{}\\{}", r.hive, resolved_key), "/f"])
                .status();
        }
    }

    if let Some(app_key) = &manifest.app.registry_key {
        let resolved_app_key = resolve_uninstall_var_string(app_key, manifest, &install_dir).replace('/', "\\");
        let full_key = format!("SOFTWARE\\{}", resolved_app_key);
        let _ = Command::new("reg")
            .args(["delete", &format!("HKLM\\{}", full_key), "/f"])
            .status();
        let _ = Command::new("reg")
            .args(["delete", &format!("HKCU\\{}", full_key), "/f"])
            .status();
    }

    progress(step_no, total_steps, "Removing shortcuts");
    step_no += 1;
    remove_all_manifest_shortcuts(manifest, &install_dir);

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
                variables: manifest.variables.as_ref()
                    .map(|v| v.resolve_for_os(std::env::consts::OS))
                    .unwrap_or_default(),
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

#[cfg(windows)]
struct LaunchCommand {
    executable: String,
    arguments: Option<String>,
    working_dir: Option<String>,
}

#[cfg(windows)]
fn build_launch_command(
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
    selected_components: &HashSet<String>,
) -> Option<LaunchCommand> {
    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::Shortcut(s) = step {
            if !component_is_selected(&s.component, selected_components) {
                continue;
            }

            let target = resolve_uninstall_var_string(&s.target, manifest, install_dir);
            if !target.to_ascii_lowercase().ends_with(".exe") {
                continue;
            }

            let args = s
                .arguments
                .as_ref()
                .map(|a| resolve_uninstall_var_string(a, manifest, install_dir));
            let work_dir = s
                .working_dir
                .as_ref()
                .map(|w| resolve_uninstall_var_string(w, manifest, install_dir));

            return Some(LaunchCommand {
                executable: target,
                arguments: args,
                working_dir: work_dir,
            });
        }
    }

    let default_exe = install_dir.join(format!("{}.exe", manifest.app.name));
    if default_exe.exists() {
        return Some(LaunchCommand {
            executable: default_exe.to_string_lossy().to_string(),
            arguments: None,
            working_dir: Some(install_dir.to_string_lossy().to_string()),
        });
    }

    None
}

#[cfg(windows)]
fn remove_desktop_shortcuts_from_manifest(
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
    selected_components: &HashSet<String>,
) -> Result<()> {
    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::Shortcut(s) = step {
            if !matches!(s.location, engine::parser::schema::ShortcutLocation::Desktop) {
                continue;
            }
            if !component_is_selected(&s.component, selected_components) {
                continue;
            }

            if let Some(path) = manifest_shortcut_path(s, manifest, install_dir) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn remove_all_manifest_shortcuts(
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
) {
    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::Shortcut(s) = step {
            if let Some(path) = manifest_shortcut_path(s, manifest, install_dir) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(windows)]
fn manifest_shortcut_path(
    step: &engine::parser::schema::ShortcutStep,
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
) -> Option<PathBuf> {
    let location = shortcut_location_path(&step.location, manifest, install_dir)?;
    let resolved_name = resolve_uninstall_var_string(&step.name, manifest, install_dir);
    Some(location.join(format!("{}.lnk", resolved_name)))
}

#[cfg(windows)]
fn shortcut_location_path(
    location: &engine::parser::schema::ShortcutLocation,
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
) -> Option<PathBuf> {
    match location {
        engine::parser::schema::ShortcutLocation::Desktop => known_folder_path("Desktop"),
        engine::parser::schema::ShortcutLocation::StartMenu => known_folder_path("StartMenu"),
        engine::parser::schema::ShortcutLocation::Startup => known_folder_path("Startup"),
        engine::parser::schema::ShortcutLocation::Custom(path) => {
            Some(PathBuf::from(resolve_uninstall_var_string(path, manifest, install_dir)))
        }
    }
}

#[cfg(windows)]
fn known_folder_path(name: &str) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::SHGetKnownFolderPath;
    use windows::core::GUID;

    let guid: GUID = match name {
        "Desktop" => GUID::from_values(
            0xB4BFCC3A,
            0xDB2C,
            0x424C,
            [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41],
        ),
        "StartMenu" => GUID::from_values(
            0x625B53C3,
            0xAB48,
            0x4EC1,
            [0xBA, 0x1F, 0xA1, 0xEF, 0x41, 0x46, 0xFC, 0x19],
        ),
        "Startup" => GUID::from_values(
            0xB97D20BB,
            0xF46A,
            0x4C97,
            [0xBA, 0x10, 0x5E, 0x36, 0x08, 0x43, 0x08, 0x54],
        ),
        _ => return None,
    };

    unsafe {
        SHGetKnownFolderPath(&guid, Default::default(), None)
            .ok()
            .map(|p| PathBuf::from(p.to_string().unwrap_or_default()))
    }
}

#[cfg(windows)]
fn component_is_selected(component: &Option<String>, selected_components: &HashSet<String>) -> bool {
    component
        .as_ref()
        .map(|id| selected_components.contains(id))
        .unwrap_or(true)
}

#[cfg(windows)]
fn resolve_uninstall_var_string(
    input: &str,
    manifest: &engine::parser::schema::InstallerManifest,
    install_dir: &PathBuf,
) -> String {
    let mut s = input.to_string();

    for _ in 0..10 {
        let before = s.clone();
        if let Some(vars) = &manifest.variables {
            let resolved = vars.resolve_for_os(std::env::consts::OS);
            for (key, value) in &resolved {
                let normalized = key.trim().trim_start_matches('$');
                if normalized.is_empty() {
                    continue;
                }
                let token_dollar = format!("${}", normalized);
                let token_template = format!("{{{{{}}}}}", normalized);
                s = s.replace(&token_dollar, value);
                s = s.replace(&token_template, value);
            }
        }
        if s == before {
            break;
        }
    }

    let install_dir_s = install_dir.to_string_lossy().to_string();
    s = s.replace("$INSTDIR", &install_dir_s);
    s = s.replace("{{INSTDIR}}", &install_dir_s);

    let pf64 = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or_else(|_| "C:\\Program Files".to_string());
    s = s.replace("$PROGRAMFILES64", &pf64);
    s = s.replace("{{PROGRAMFILES64}}", &pf64);

    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    s = s.replace("$PROGRAMFILES", &pf);
    s = s.replace("{{PROGRAMFILES}}", &pf);

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    s = s.replace("$APPDATA", &appdata);
    s = s.replace("{{APPDATA}}", &appdata);

    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    s = s.replace("$LOCALAPPDATA", &local);
    s = s.replace("{{LOCALAPPDATA}}", &local);

    let temp = std::env::var("TEMP").unwrap_or_default();
    s = s.replace("$TEMP", &temp);
    s = s.replace("{{TEMP}}", &temp);

    let windir = std::env::var("WINDIR").unwrap_or_default();
    s = s.replace("$WINDIR", &windir);
    s = s.replace("{{WINDIR}}", &windir);
    s
}