use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::Path;

/// Decompresses a Zstd-compressed tar archive from `data` into `destination`.
/// Reports progress via the `on_progress` callback: (bytes_written, total_bytes_estimate).
pub fn extract_zstd_archive(
    data: &[u8],
    destination: &Path,
    on_progress: impl Fn(u64, u64) + Send + Sync,
) -> Result<Vec<String>> {
    use std::io::Cursor;

    std::fs::create_dir_all(destination)
        .with_context(|| format!("Failed to create destination: {}", destination.display()))?;

    // Decompress Zstd stream
    let cursor = Cursor::new(data);
    let mut decoder = zstd::Decoder::new(cursor)
        .context("Failed to create Zstd decoder")?;

    // Read fully decompressed bytes (tar archive)
    let mut tar_data = Vec::new();
    decoder.read_to_end(&mut tar_data)
        .context("Zstd decompression failed")?;

    let total = tar_data.len() as u64;
    let mut archive = tar::Archive::new(Cursor::new(&tar_data));
    let mut extracted_files = Vec::new();
    let mut bytes_done: u64 = 0;

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Corrupt tar entry")?;
        let entry_path = entry.path().context("Invalid entry path")?.to_path_buf();
        let dest_path = destination.join(&entry_path);

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&dest_path)
                .with_context(|| format!("mkdir failed: {}", dest_path.display()))?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&dest_path)
                .with_context(|| format!("Cannot create file: {}", dest_path.display()))?;

            let mut buf = [0u8; 65536];
            loop {
                let n = entry.read(&mut buf).context("Read error during extraction")?;
                if n == 0 { break; }
                file.write_all(&buf[..n])?;
                bytes_done += n as u64;
                on_progress(bytes_done, total);
            }
            extracted_files.push(dest_path.to_string_lossy().into_owned());
        }
    }

    Ok(extracted_files)
}