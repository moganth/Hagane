use anyhow::Result;

/// Returns free disk space in megabytes for the volume containing `path`.
pub fn get_free_disk_mb(path: &str) -> Result<u64> {
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut free_bytes_caller: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut free_bytes_total: u64 = 0;

        // SAFETY: wide is null-terminated, pointers are valid u64 locations.
        unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(wide.as_ptr()),
                Some(&mut free_bytes_caller),
                Some(&mut total_bytes),
                Some(&mut free_bytes_total),
            )
            .map_err(|e| anyhow::anyhow!("GetDiskFreeSpaceExW failed for '{}': {}", path, e))?;
        }

        Ok(free_bytes_caller / (1024 * 1024))
    }
    #[cfg(not(windows))]
    {
        // Stub
        let _ = path;
        Ok(100_000)
    }
}

/// Returns true if `path`'s volume has at least min_mb free.
pub fn meets_disk_requirement(path: &str, min_mb: u64) -> Result<bool> {
    Ok(get_free_disk_mb(path)? >= min_mb)
}