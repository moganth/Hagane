use anyhow::Result;

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub platform: String,
}

/// Reads the real Windows version using RtlGetVersion (bypasses compatibility shims).
/// Falls back to a stub on non-Windows builds for compilation purposes.
pub fn get_os_info() -> Result<OsInfo> {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::OSVERSIONINFOEXW;

        // RtlGetVersion is in ntdll.dll but not exposed via the windows crate's
        // SystemInformation feature. Declare it manually — stable since XP.
        #[link(name = "ntdll")]
        extern "system" {
            fn RtlGetVersion(lpVersionInformation: *mut OSVERSIONINFOEXW) -> i32;
        }

        let mut info = OSVERSIONINFOEXW::default();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXW>() as u32;

        // SAFETY: info is correctly sized; RtlGetVersion always returns STATUS_SUCCESS.
        unsafe { RtlGetVersion(&mut info as *mut _) };

        Ok(OsInfo {
            major: info.dwMajorVersion,
            minor: info.dwMinorVersion,
            build: info.dwBuildNumber,
            platform: "windows".to_string(),
        })
    }
    #[cfg(not(windows))]
    {
        // Non-Windows stub — used only during cross-compilation / CI
        Ok(OsInfo {
            major: 0,
            minor: 0,
            build: 0,
            platform: std::env::consts::OS.to_string(),
        })
    }
}

/// Returns true if the current Windows build is >= min_build.
pub fn meets_build_requirement(min_build: u32) -> Result<bool> {
    let info = get_os_info()?;
    Ok(info.build >= min_build)
}

/// Well-known Windows build numbers for convenience.
pub mod builds {
    pub const WIN10_RTM: u32 = 10240;
    pub const WIN10_1903: u32 = 18362;
    pub const WIN11_RTM: u32 = 22000;
    pub const WIN11_22H2: u32 = 22621;
}