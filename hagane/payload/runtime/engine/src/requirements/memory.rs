use anyhow::Result;

/// Returns total physical RAM in megabytes.
pub fn get_total_ram_mb() -> Result<u64> {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        let mut status = MEMORYSTATUSEX::default();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

        // SAFETY: dwLength is set correctly before the call.
        unsafe {
            GlobalMemoryStatusEx(&mut status)
                .map_err(|e| anyhow::anyhow!("GlobalMemoryStatusEx failed: {}", e))?;
        }

        Ok(status.ullTotalPhys / (1024 * 1024))
    }
    #[cfg(not(windows))]
    {
        // Stub for non-Windows compilation
        Ok(8192)
    }
}

/// Returns true if the system has at least min_mb of physical RAM.
pub fn meets_ram_requirement(min_mb: u64) -> Result<bool> {
    Ok(get_total_ram_mb()? >= min_mb)
}