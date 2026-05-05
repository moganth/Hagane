// ── wry / tao GUI for non-Windows platforms ───────────────────────────────────
//
// This module mirrors the WebView2 GUI path in main.rs but uses wry + tao
// (WebKitGTK on Linux). The JavaScript bridge is identical; only the Rust side
// of the IPC changes.
//
// Required system package (Ubuntu/Debian):
//   sudo apt install libwebkit2gtk-4.1-dev

use anyhow::Result;
use engine::{
    install::{InstallContext, StepRunner},
    ipc::{parse_inbound, InboundMessage, OutboundEvent},
    parser::schema::InstallerManifest,
    requirements,
    state::{InstallProgress, InstallerState, Page},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::unix::WindowExtUnix,
    window::WindowBuilder,
};
use wry::{WebViewBuilder, WebViewBuilderExtUnix};

/// Custom user event — used to push Rust→JS messages from background threads
/// back onto the main (UI) thread.
#[derive(Debug)]
pub enum AppEvent {
    Eval(String),
    CloseWindow,
}

pub fn run_gui(
    manifest: InstallerManifest,
    state: InstallerState,
    archives: HashMap<String, Vec<u8>>,
    html_map: HashMap<String, String>,
) -> Result<()> {
    let state = Arc::new(Mutex::new(state));
    let manifest = Arc::new(manifest);
    let archives = Arc::new(archives);
    let html_map = Arc::new(html_map);

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let (win_w, win_h, title) = {
        let st = state.lock().unwrap();
        let t = if st.is_uninstall {
            format!("{} — Uninstall", st.app_name)
        } else {
            format!("{} — Setup", st.app_name)
        };
        (st.window_width, st.window_height, t)
    };

    let window = WindowBuilder::new()
        .with_title(&title)
        .with_inner_size(LogicalSize::new(win_w, win_h))
        .with_resizable(true)
        .build(&event_loop)?;

    let shell_html = html_map
        .get("shell")
        .cloned()
        .unwrap_or_else(|| "<html><body>Missing shell.html</body></html>".into());

    // Clone Arcs for the IPC handler closure.
    let state_ipc     = Arc::clone(&state);
    let manifest_ipc  = Arc::clone(&manifest);
    let archives_ipc  = Arc::clone(&archives);
    let html_map_ipc  = Arc::clone(&html_map);
    let proxy_ipc     = proxy.clone();

    let webview = WebViewBuilder::new_gtk(window.default_vbox().unwrap())
        .with_html(shell_html)
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |request: wry::http::Request<String>| {
            let msg = request.into_body();
            handle_message(
                msg,
                &proxy_ipc,
                &state_ipc,
                &manifest_ipc,
                &archives_ipc,
                &html_map_ipc,
            );
        })
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(AppEvent::Eval(js)) => {
                let _ = webview.evaluate_script(&js);
            }

            Event::UserEvent(AppEvent::CloseWindow) => {
                *control_flow = ControlFlow::Exit;
            }

            _ => {}
        }
    });
}

// ── IPC message handler ───────────────────────────────────────────────────────

fn handle_message(
    raw: String,
    proxy: &tao::event_loop::EventLoopProxy<AppEvent>,
    state: &Arc<Mutex<InstallerState>>,
    manifest: &Arc<InstallerManifest>,
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
            send_state(proxy, &st, true);
            let page_name = page_to_filename(st.current_page()).to_string();
            if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
                send_event(proxy, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
            }
        }

        InboundMessage::GetState => {
            send_state(proxy, &st, false);
        }

        InboundMessage::Next => {
            // ── Install page: trigger installation ────────────────────────
            if matches!(st.current_page(), Page::Install) {
                if st.install_succeeded.is_none() {
                    let total = manifest.steps.len();
                    st.progress = Some(InstallProgress::new(total));
                    st.install_error = None;

                    let install_dir      = st.install_dir.clone();
                    let selected         = st.selected_components.clone();
                    let custom_values    = st.custom_values.clone();
                    let state_clone      = Arc::clone(state);
                    let manifest_clone   = Arc::clone(manifest);
                    let archives_clone   = Arc::clone(archives);
                    let proxy_clone      = proxy.clone();
                    let initial_state    = to_state_json(&st, false);
                    drop(st);

                    std::thread::spawn(move || {
                        let ctx = InstallContext {
                            install_dir: PathBuf::from(&install_dir),
                            selected_components: selected,
                            archives: (*archives_clone).clone(),
                            backup_dir: std::env::temp_dir().join("installer_backup"),
                            logging: manifest_clone.logging.clone(),
                            variables: {
                                let mut vars = manifest_clone.variables.as_ref()
                                    .map(|v| v.resolve_for_os(std::env::consts::OS))
                                    .unwrap_or_default();
                                vars.extend(custom_values);
                                vars
                            },
                        };

                        let mut runner = StepRunner::new(ctx);
                        let result = runner.run_all(&manifest_clone.steps, |step, total, label| {
                            let mut s = state_clone.lock().unwrap();
                            if s.progress.is_none() {
                                s.progress = Some(InstallProgress::new(total));
                            }
                            if let Some(p) = s.progress.as_mut() {
                                let n = if step < total { step + 1 } else { total };
                                p.update(n, total, label);
                            }
                            // Push a state update to the UI thread.
                            let json = to_state_json(&s, false);
                            let js = OutboundEvent::StateUpdate { state: json }.to_js_call();
                            let _ = proxy_clone.send_event(AppEvent::Eval(js));
                        });

                        let mut s = state_clone.lock().unwrap();
                        match result {
                            Ok(()) => {
                                s.install_succeeded = Some(true);
                                s.install_error = None;
                                if let Some(p) = s.progress.as_mut() {
                                    p.percent = 100;
                                    p.current_label = "Done".to_string();
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
                        let json = to_state_json(&s, false);
                        let js = OutboundEvent::StateUpdate { state: json }.to_js_call();
                        let _ = proxy_clone.send_event(AppEvent::Eval(js));
                    });

                    send_event(proxy, &OutboundEvent::StateUpdate { state: initial_state });
                    return;
                }
            }

            // ── Terminal pages ────────────────────────────────────────────
            if matches!(st.current_page(), Page::Finish | Page::Error) {
                let _ = proxy.send_event(AppEvent::CloseWindow);
                return;
            }

            // ── Requirements page: check before advancing ─────────────────
            // Peek at what the next page would be without advancing yet.
            let peek_idx = st.current_page_index + 1;
            let peek_is_requirements = st.pages.get(peek_idx)
                .map(|p| matches!(p, Page::Requirements))
                .unwrap_or(false);

            if peek_is_requirements {
                let reqs = manifest.requirements.clone();
                let install_dir = st.install_dir.clone();
                let state_clone = Arc::clone(state);
                let proxy_clone = proxy.clone();
                let html_clone  = Arc::clone(html_map);
                let theme_preset = st.theme_preset.clone();
                // Advance to Requirements now.
                st.go_next();
                let next_page_name = page_to_filename(st.current_page()).to_string();
                drop(st);

                std::thread::spawn(move || {
                    let results = if let Some(r) = &reqs {
                        requirements::run_all(r, &install_dir)
                    } else {
                        vec![]
                    };
                    let all_passed = results.iter().all(|r| r.passed);
                    {
                        let mut s = state_clone.lock().unwrap();
                        s.requirement_results = results;
                        s.requirements_passed = all_passed;
                    }
                    let s = state_clone.lock().unwrap();
                    if let Some(html) = select_page_html(&html_clone, &next_page_name, &theme_preset) {
                        let ev = OutboundEvent::Navigate { page: next_page_name.clone(), html: html.clone() };
                        let _ = proxy_clone.send_event(AppEvent::Eval(ev.to_js_call()));
                    }
                    let json = to_state_json(&s, false);
                    let js = OutboundEvent::StateUpdate { state: json }.to_js_call();
                    let _ = proxy_clone.send_event(AppEvent::Eval(js));
                });
                return;
            }

            if st.can_go_next() {
                st.go_next();
                let page_name = page_to_filename(st.current_page()).to_string();
                if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
                    send_event(proxy, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                }
                send_state(proxy, &st, false);
            }
        }

        InboundMessage::Back => {
            if st.can_go_back() {
                st.go_back();
                let page_name = page_to_filename(st.current_page()).to_string();
                if let Some(html) = select_page_html(html_map, &page_name, &st.theme_preset) {
                    send_event(proxy, &OutboundEvent::Navigate { page: page_name, html: html.clone() });
                }
                send_state(proxy, &st, false);
            }
        }

        InboundMessage::Cancel => {
            let _ = proxy.send_event(AppEvent::CloseWindow);
        }

        InboundMessage::LicenseAccepted { accepted } => {
            st.license.accepted = accepted;
            send_state(proxy, &st, false);
        }

        InboundMessage::SetInstallDir { path } => {
            st.install_dir = path;
            send_state(proxy, &st, false);
        }

        InboundMessage::SetCustomValue { id, value } => {
            st.set_custom_value(&id, value);
            send_state(proxy, &st, false);
        }

        InboundMessage::SetComponent { id, selected } => {
            if selected { st.selected_components.insert(id); }
            else        { st.selected_components.remove(&id); }
            send_state(proxy, &st, false);
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
        }

        InboundMessage::BrowseInstallDir | InboundMessage::BrowseFolder { .. } => {
            // File chooser dialogs require GTK main thread integration.
            // For now send an empty result; a future pass can add rfd / gtk dialog.
            drop(st);
            send_event(proxy, &OutboundEvent::BrowseResult { id: None, path: None });
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn send_event(proxy: &tao::event_loop::EventLoopProxy<AppEvent>, event: &OutboundEvent) {
    let js = event.to_js_call();
    let _ = proxy.send_event(AppEvent::Eval(js));
}

fn send_state(
    proxy: &tao::event_loop::EventLoopProxy<AppEvent>,
    st: &InstallerState,
    include_assets: bool,
) {
    send_event(proxy, &OutboundEvent::StateUpdate {
        state: to_state_json(st, include_assets),
    });
}

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

fn page_to_filename(page: &Page) -> &'static str {
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

fn select_page_html<'a>(
    map: &'a HashMap<String, String>,
    page_name: &str,
    preset: &str,
) -> Option<&'a String> {
    if !preset.is_empty() {
        let key = format!("{}__theme__{}", page_name, preset);
        if let Some(html) = map.get(&key) {
            return Some(html);
        }
    }
    map.get(page_name)
}

fn open_external_url(url: &str) {
    // xdg-open is available on all freedesktop-compliant distros.
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
