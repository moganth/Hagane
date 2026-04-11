use anyhow::{Context, Result};
use std::path::Path;
use super::rollback::{JournalEntry, RollbackJournal};

/// Copy a file from source to destination, journaling the operation.
pub fn copy_file(
    source: &Path,
    destination: &Path,
    overwrite: bool,
    journal: &mut RollbackJournal,
) -> Result<()> {
    if destination.exists() {
        if !overwrite {
            log::info!("Skipping existing file: {}", destination.display());
            return Ok(());
        }
        // Back up existing file before overwrite
        let backup_name = format!(
            "{}.bak",
            destination.file_name().unwrap_or_default().to_string_lossy()
        );
        let backup_path = journal.backup_dir().join(&backup_name);
        std::fs::copy(destination, &backup_path)
            .with_context(|| format!("Backup failed for: {}", destination.display()))?;
        journal.record(JournalEntry::FileBackedUp {
            original: destination.to_string_lossy().into(),
            backup: backup_path.to_string_lossy().into(),
        });
    }

    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir failed: {}", parent.display()))?;
            journal.record(JournalEntry::DirCreated { path: parent.to_string_lossy().into() });
        }
    }

    std::fs::copy(source, destination)
        .with_context(|| format!("Copy failed: {} → {}", source.display(), destination.display()))?;

    journal.record(JournalEntry::FileCreated { path: destination.to_string_lossy().into() });
    log::debug!("Copied: {} → {}", source.display(), destination.display());
    Ok(())
}

/// Create a directory (and parents), journaling the creation.
pub fn create_dir(path: &Path, journal: &mut RollbackJournal) -> Result<()> {
    if path.exists() { return Ok(()); }
    std::fs::create_dir_all(path)
        .with_context(|| format!("mkdir failed: {}", path.display()))?;
    journal.record(JournalEntry::DirCreated { path: path.to_string_lossy().into() });
    Ok(())
}

/// Delete a file (no journal needed for delete — it's irreversible by design).
pub fn delete_file(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Delete failed: {}", path.display()))?;
    }
    Ok(())
}