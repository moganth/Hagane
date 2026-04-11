use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Compress the contents of `source_dir` into a Zstd-compressed tar archive.
/// Returns the compressed bytes.
pub fn compress_directory(source_dir: &Path, compression_level: i32) -> Result<Vec<u8>> {
    log::info!("Compressing: {} (level {})", source_dir.display(), compression_level);

    let mut tar_builder = tar::Builder::new(Vec::new());

    for entry in WalkDir::new(source_dir).min_depth(1).into_iter() {
        let entry = entry.context("WalkDir error")?;
        let rel_path = entry.path().strip_prefix(source_dir)
            .context("strip_prefix failed")?;

        if entry.file_type().is_dir() {
            tar_builder.append_dir(rel_path, entry.path())
                .with_context(|| format!("append_dir failed: {}", rel_path.display()))?;
        } else if entry.file_type().is_file() {
            let mut file = std::fs::File::open(entry.path())
                .with_context(|| format!("open failed: {}", entry.path().display()))?;
            tar_builder.append_file(rel_path, &mut file)
                .with_context(|| format!("append_file failed: {}", rel_path.display()))?;
            log::debug!("  + {}", rel_path.display());
        }
    }

    let tar_data = tar_builder.into_inner().context("tar finalize failed")?;
    let uncompressed_size = tar_data.len();

    // Compress with Zstd
    let compressed = zstd::encode_all(tar_data.as_slice(), compression_level)
        .context("Zstd compression failed")?;

    let ratio = compressed.len() as f64 / uncompressed_size as f64 * 100.0;
    log::info!(
        "Compressed: {} bytes → {} bytes ({:.1}%)",
        uncompressed_size, compressed.len(), ratio
    );

    Ok(compressed)
}

/// Compress a single file into a Zstd-compressed tar archive.
pub fn compress_file(source_file: &Path, compression_level: i32) -> Result<Vec<u8>> {
    log::info!("Compressing file: {}", source_file.display());
    let mut tar_builder = tar::Builder::new(Vec::new());
    let mut file = std::fs::File::open(source_file)
        .with_context(|| format!("open failed: {}", source_file.display()))?;
    let filename = source_file.file_name().unwrap_or_default();
    tar_builder.append_file(filename, &mut file)?;
    let tar_data = tar_builder.into_inner()?;
    zstd::encode_all(tar_data.as_slice(), compression_level).context("Zstd compression failed")
}