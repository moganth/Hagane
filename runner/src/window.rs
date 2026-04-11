use anyhow::{Context, Result};

#[cfg(windows)]
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
};

#[cfg(windows)]
pub fn create_window(title: &str, width: u32, height: u32) -> Result<HWND> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null())
            .context("GetModuleHandleW failed")?;

        let class_name: Vec<u16> = OsStr::new("InstallerEngineWindow")
            .encode_wide().chain(std::iter::once(0)).collect();
        let window_title: Vec<u16> = OsStr::new(title)
            .encode_wide().chain(std::iter::once(0)).collect();

        let class_icon = load_window_icon(hinstance);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hIcon: class_icon,
            hIconSm: class_icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(COLOR_BTNFACE.0 as isize + 1),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - width as i32) / 2;
        let y = (screen_h - height as i32) / 2;

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut rect = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        AdjustWindowRect(&mut rect, style, false)?;
        let actual_w = rect.right - rect.left;
        let actual_h = rect.bottom - rect.top;

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            style,
            x, y, actual_w, actual_h,
            None, None, hinstance, None,
        );
        if hwnd.0 == 0 {
            anyhow::bail!("CreateWindowExW failed");
        }

        if !class_icon.is_invalid() {
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(class_icon.0));
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(class_icon.0));
        }
        ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        Ok(hwnd)
    }
}

#[cfg(windows)]
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        WM_CLOSE   => { let _ = DestroyWindow(hwnd); LRESULT(0) }
        _          => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
pub fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
pub fn run_message_loop() {}

#[cfg(not(windows))]
pub fn create_window(_title: &str, _width: u32, _height: u32) -> anyhow::Result<()> {
    anyhow::bail!("create_window only supported on Windows")
}

#[cfg(windows)]
fn load_window_icon(hinstance: windows::Win32::Foundation::HMODULE) -> HICON {
    unsafe {
        // First try icon embedded by winres in the executable resources.
        let instance = windows::Win32::Foundation::HINSTANCE::from(hinstance);
        if let Ok(res_icon) = LoadIconW(instance, windows::core::PCWSTR(1 as _)) {
            if !res_icon.is_invalid() {
                return res_icon;
            }
        }

        // Fallback for local dev runs where a resource icon may not be present.
        let fallback = std::path::Path::new("sdk/example/assets/icon.ico");
        if fallback.exists() {
            let wide: Vec<u16> = fallback
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            if let Ok(h) = windows::Win32::UI::WindowsAndMessaging::LoadImageW(
                None,
                windows::core::PCWSTR(wide.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::IMAGE_ICON,
                32,
                32,
                windows::Win32::UI::WindowsAndMessaging::LR_LOADFROMFILE,
            ) {
                if !h.is_invalid() {
                    return HICON(h.0);
                }
            }
        }

        HICON::default()
    }
}