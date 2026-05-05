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

/// Parses `/etc/os-release` on Linux to return (distro_id, version_id).
/// Returns `("linux", "")` if the file is missing or unparseable.
#[cfg(target_os = "linux")]
pub fn get_linux_os_info() -> (String, String) {
    let content = match std::fs::read_to_string("/etc/os-release") {
        Ok(c) => c,
        Err(_) => return ("linux".to_string(), String::new()),
    };
    let mut id = String::new();
    let mut version = String::new();
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("ID=") {
            id = val.trim_matches('"').to_lowercase();
        } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
            version = val.trim_matches('"').to_string();
        }
    }
    if id.is_empty() { id = "linux".to_string(); }
    (id, version)
}

#[cfg(not(target_os = "linux"))]
pub fn get_linux_os_info() -> (String, String) {
    (std::env::consts::OS.to_string(), String::new())
}

/// Returns true if the current Linux distro/version meets the requirement.
/// `allowed_distros` is empty-or-any; `min_version` is empty-or-semver-like.
pub fn meets_linux_requirement(allowed_distros: &[String], min_version: &str) -> anyhow::Result<bool> {
    let (distro, version) = get_linux_os_info();

    if !allowed_distros.is_empty() {
        let matched = allowed_distros.iter().any(|d| d.to_lowercase() == distro);
        if !matched {
            return Ok(false);
        }
    }

    if !min_version.is_empty() && !version.is_empty() {
        // Simple dotted-numeric comparison: "20.04" ≤ "22.04"
        let ok = compare_version_strings(&version, min_version);
        return Ok(ok);
    }

    Ok(true)
}

/// Returns true if `actual` >= `required` (dotted-numeric comparison).
fn compare_version_strings(actual: &str, required: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    let a = parse(actual);
    let r = parse(required);
    for i in 0..r.len().max(a.len()) {
        let av = a.get(i).copied().unwrap_or(0);
        let rv = r.get(i).copied().unwrap_or(0);
        if av != rv { return av > rv; }
    }
    true // equal
}