use anyhow::{bail, Context, Result};
use crate::parser::schema::{RegistryOperation, RegistryValueType};
use super::rollback::{JournalEntry, RollbackJournal};

pub fn apply_registry_step(
    operation: &RegistryOperation,
    hive: &str,
    key: &str,
    value_name: Option<&str>,
    value_type: Option<&RegistryValueType>,
    value_data: Option<&serde_json::Value>,
    journal: &mut RollbackJournal,
) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::*;
        use windows::core::PCWSTR;

        let root_hkey = parse_hive(hive)?;

        match operation {
            RegistryOperation::Write | RegistryOperation::CreateKey => {
                let wide_key: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
                let mut hkey = HKEY::default();
                let mut disposition = REG_CREATE_KEY_DISPOSITION::default();

                unsafe {
                    RegCreateKeyExW(
                        root_hkey,
                        PCWSTR(wide_key.as_ptr()),
                        0, None, REG_OPTION_NON_VOLATILE,
                        KEY_WRITE, None,
                        &mut hkey, Some(&mut disposition),
                    ).ok().context("RegCreateKeyExW failed")?;
                }

                if disposition == REG_CREATED_NEW_KEY {
                    journal.record(JournalEntry::RegistryKeyCreated {
                        hive: hive.into(), key: key.into(),
                    });
                }

                if matches!(operation, RegistryOperation::Write) {
                    if let (Some(vname), Some(vtype), Some(vdata)) = (value_name, value_type, value_data) {
                        write_value(hkey, vname, vtype, vdata)?;
                        journal.record(JournalEntry::RegistryWritten {
                            hive: hive.into(), key: key.into(),
                            value_name: Some(vname.into()),
                        });
                    }
                }

                unsafe { RegCloseKey(hkey).ok().context("RegCloseKey failed")?; }
            }

            RegistryOperation::Delete => {
                let wide_key: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
                let mut hkey = HKEY::default();
                let open_res = unsafe {
                    RegOpenKeyExW(root_hkey, PCWSTR(wide_key.as_ptr()), 0, KEY_WRITE, &mut hkey)
                };
                if !open_res.is_ok() { return Ok(()); } // Key doesn't exist, nothing to do

                if let Some(vname) = value_name {
                    let wide_val: Vec<u16> = vname.encode_utf16().chain(std::iter::once(0)).collect();
                    unsafe {
                        RegDeleteValueW(hkey, PCWSTR(wide_val.as_ptr()))
                            .ok().context("RegDeleteValueW failed")?;
                    }
                }
                unsafe { RegCloseKey(hkey).context("RegCloseKey failed")?; }
            }

            RegistryOperation::DeleteKey => {
                let wide_key: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    RegDeleteKeyW(root_hkey, PCWSTR(wide_key.as_ptr())).context("RegDeleteKeyW failed")?;
                }
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        log::warn!("Registry operations skipped on non-Windows");
        Ok(())
    }
}

#[cfg(windows)]
fn write_value(
    hkey: windows::Win32::System::Registry::HKEY,
    name: &str,
    vtype: &RegistryValueType,
    data: &serde_json::Value,
) -> Result<()> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

    let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

    match vtype {
        RegistryValueType::Sz | RegistryValueType::ExpandSz => {
            let s = data.as_str().unwrap_or_default();
            let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
            let reg_type = if matches!(vtype, RegistryValueType::ExpandSz) { REG_EXPAND_SZ } else { REG_SZ };
            unsafe {
                RegSetValueExW(
                    hkey, PCWSTR(wide_name.as_ptr()), 0, reg_type,
                    Some(std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)),
                ).ok().context("RegSetValueExW (SZ) failed")?;
            }
        }
        RegistryValueType::Dword => {
            let val = data.as_u64().unwrap_or(0) as u32;
            unsafe {
                RegSetValueExW(
                    hkey, PCWSTR(wide_name.as_ptr()), 0, REG_DWORD,
                    Some(&val.to_le_bytes()),
                ).ok().context("RegSetValueExW (DWORD) failed")?;
            }
        }
        RegistryValueType::Qword => {
            let val = data.as_u64().unwrap_or(0);
            unsafe {
                RegSetValueExW(
                    hkey, PCWSTR(wide_name.as_ptr()), 0, REG_QWORD,
                    Some(&val.to_le_bytes()),
                ).ok().context("RegSetValueExW (QWORD) failed")?;
            }
        }
        _ => bail!("Unsupported registry value type for writing"),
    }
    Ok(())
}

#[cfg(windows)]
fn parse_hive(hive: &str) -> Result<windows::Win32::System::Registry::HKEY> {
    use windows::Win32::System::Registry::*;
    match hive {
        "HKLM" => Ok(HKEY_LOCAL_MACHINE),
        "HKCU" => Ok(HKEY_CURRENT_USER),
        "HKCR" => Ok(HKEY_CLASSES_ROOT),
        "HKU"  => Ok(HKEY_USERS),
        "HKCC" => Ok(HKEY_CURRENT_CONFIG),
        _ => bail!("Unknown registry hive: {}", hive),
    }
}