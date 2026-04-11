use anyhow::Result;

/// Check for .NET Framework >= min_version (e.g. "4.8") using registry.
/// Key: HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full  →  Release DWORD
pub fn check_dotnet_framework(min_version: &str) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, REG_VALUE_TYPE,
        };
        use windows::core::PCWSTR;

        // Map human version strings to minimum Release DWORD values
        let min_release: u32 = match min_version {
            "4.5"   => 378389,
            "4.5.1" => 378675,
            "4.5.2" => 379893,
            "4.6"   => 393295,
            "4.6.1" => 394254,
            "4.6.2" => 394802,
            "4.7"   => 460798,
            "4.7.1" => 461308,
            "4.7.2" => 461808,
            "4.8"   => 528040,
            "4.8.1" => 533320,
            _ => return Err(anyhow::anyhow!("Unknown .NET version: {}", min_version)),
        };

        let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\NET Framework Setup\\NDP\\v4\\Full"
            .encode_utf16().chain(std::iter::once(0)).collect();
        let value_name: Vec<u16> = "Release".encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            )
        };

        if !result.is_ok() {
            return Ok(false); // Key doesn't exist → .NET not installed
        }

        let mut data: u32 = 0;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let mut reg_type = REG_VALUE_TYPE::default();

        let qresult = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut reg_type),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_size),
            )
        };

        let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(hkey) };

        if !qresult.is_ok() {
            return Ok(false);
        }

        Ok(data >= min_release)
    }
    #[cfg(not(windows))]
    {
        let _ = min_version;
        Ok(true) // stub
    }
}

/// Check for VC++ Redistributable by scanning known uninstall registry keys.
pub fn check_vc_redist(year: &str, arch: Option<&str>) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            RegOpenKeyExW, RegEnumKeyExW, HKEY_LOCAL_MACHINE, KEY_READ,
        };
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::ERROR_SUCCESS;

        let arch_str = arch.unwrap_or("x64");
        // Search both 32-bit and 64-bit uninstall keys
        let uninstall_paths = [
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ];

        for path in &uninstall_paths {
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut hkey = windows::Win32::System::Registry::HKEY::default();

            let res = unsafe {
                RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(wide.as_ptr()), 0, KEY_READ, &mut hkey)
            };
            if !res.is_ok() { continue; }

            let mut index = 0u32;
            loop {
                let mut name_buf = vec![0u16; 256];
                let mut name_len = 256u32;
                let enum_res = unsafe {
                    RegEnumKeyExW(
                        hkey,
                        index,
                        windows::core::PWSTR(name_buf.as_mut_ptr()),
                        &mut name_len,
                        None,
                        windows::core::PWSTR::null(),
                        Some(std::ptr::null_mut()),
                        None,
                    )
                };
                if !enum_res.is_ok() { break; }
                index += 1;

                let key_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                // VC++ keys contain "Microsoft Visual C++" and the year
                if key_name.contains("Microsoft Visual C++")
                    && key_name.contains(year)
                    && key_name.to_lowercase().contains(arch_str)
                {
                    let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(hkey) };
                    return Ok(true);
                }
            }
            let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(hkey) };
        }
        Ok(false)
    }
    #[cfg(not(windows))]
    {
        let _ = (year, arch);
        Ok(true) // stub
    }
}