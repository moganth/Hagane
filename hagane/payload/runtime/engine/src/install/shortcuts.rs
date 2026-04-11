use anyhow::{Context, Result};
use super::rollback::{JournalEntry, RollbackJournal};
use crate::parser::schema::ShortcutLocation;

/// Creates a Windows .lnk shortcut file using the COM IShellLink interface.
pub fn create_shortcut(
    target: &str,
    location: &ShortcutLocation,
    name: &str,
    description: Option<&str>,
    icon: Option<&str>,
    arguments: Option<&str>,
    working_dir: Option<&str>,
    journal: &mut RollbackJournal,
) -> Result<()> {
    let lnk_dir = resolve_location(location)?;
    std::fs::create_dir_all(&lnk_dir)?;
    let lnk_path = lnk_dir.join(format!("{}.lnk", name));

    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize,
            IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        };
        use windows::core::ComInterface;
        use windows::core::PCWSTR;

        unsafe {
            // COM init for this thread
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .context("CoCreateInstance ShellLink failed")?;

            // Target path
            let wide_target = to_wide(target);
            shell_link.SetPath(PCWSTR(wide_target.as_ptr()))
                .context("SetPath failed")?;

            // Arguments
            if let Some(args) = arguments {
                let wide = to_wide(args);
                shell_link.SetArguments(PCWSTR(wide.as_ptr())).ok();
            }

            // Description
            if let Some(desc) = description {
                let wide = to_wide(desc);
                shell_link.SetDescription(PCWSTR(wide.as_ptr())).ok();
            }

            // Working directory
            if let Some(dir) = working_dir {
                let wide = to_wide(dir);
                shell_link.SetWorkingDirectory(PCWSTR(wide.as_ptr())).ok();
            }

            // Icon
            if let Some(ico) = icon {
                let wide = to_wide(ico);
                shell_link.SetIconLocation(PCWSTR(wide.as_ptr()), 0).ok();
            }

            // Save the .lnk file
            let persist: IPersistFile = shell_link.cast().context("IPersistFile cast failed")?;
            let wide_lnk = to_wide(lnk_path.to_str().unwrap_or_default());
            persist.Save(PCWSTR(wide_lnk.as_ptr()), true)
                .context("IPersistFile::Save failed")?;

            CoUninitialize();
        }
    }
    #[cfg(not(windows))]
    {
        // On non-Windows, write a stub file for testing
        std::fs::write(&lnk_path, format!("[shortcut]\ntarget={}", target))?;
    }

    journal.record(JournalEntry::ShortcutCreated { path: lnk_path.to_string_lossy().into() });
    log::info!("Shortcut created: {}", lnk_path.display());
    Ok(())
}

fn resolve_location(location: &ShortcutLocation) -> Result<std::path::PathBuf> {
    match location {
        ShortcutLocation::Desktop => {
            Ok(dirs_path("Desktop").unwrap_or_else(|| std::env::temp_dir().join("Desktop")))
        }
        ShortcutLocation::StartMenu => {
            Ok(dirs_path("StartMenu").unwrap_or_else(|| std::env::temp_dir().join("StartMenu")))
        }
        ShortcutLocation::Startup => {
            Ok(dirs_path("Startup").unwrap_or_else(|| std::env::temp_dir().join("Startup")))
        }
        ShortcutLocation::Custom(path) => Ok(std::path::PathBuf::from(path)),
    }
}

fn dirs_path(name: &str) -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::SHGetKnownFolderPath;
        use windows::core::GUID;

        // Known folder GUIDs
        let guid: GUID = match name {
            "Desktop"   => GUID::from_values(0xB4BFCC3A, 0xDB2C, 0x424C, [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41]),
            "StartMenu" => GUID::from_values(0x625B53C3, 0xAB48, 0x4EC1, [0xBA, 0x1F, 0xA1, 0xEF, 0x41, 0x46, 0xFC, 0x19]),
            "Startup"   => GUID::from_values(0xB97D20BB, 0xF46A, 0x4C97, [0xBA, 0x10, 0x5E, 0x36, 0x08, 0x43, 0x08, 0x54]),
            _ => return None,
        };
        unsafe {
            SHGetKnownFolderPath(&guid, Default::default(), None)
                .ok()
                .map(|p| std::path::PathBuf::from(p.to_string().unwrap_or_default()))
        }
    }
    #[cfg(not(windows))]
    { None }
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}