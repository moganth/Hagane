use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JournalEntry {
    FileCreated { path: String },
    DirCreated  { path: String },
    FileBackedUp { original: String, backup: String },
    RegistryWritten { hive: String, key: String, value_name: Option<String> },
    RegistryKeyCreated { hive: String, key: String },
    ShortcutCreated { path: String },
    EnvVarSet { name: String, scope: String, previous_value: Option<String> },
    ServiceInstalled { name: String },
}

#[derive(Debug, Default)]
pub struct RollbackJournal {
    entries: Vec<JournalEntry>,
    backup_dir: PathBuf,
}

impl RollbackJournal {
    pub fn new(backup_dir: &Path) -> Self {
        Self { entries: Vec::new(), backup_dir: backup_dir.to_path_buf() }
    }

    pub fn record(&mut self, entry: JournalEntry) {
        self.entries.push(entry);
    }

    /// Undo all journaled actions in reverse order.
    pub fn rollback(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for entry in self.entries.iter().rev() {
            if let Err(e) = self.undo(entry) {
                errors.push(format!("{:?}: {}", entry, e));
            }
        }
        errors
    }

    fn undo(&self, entry: &JournalEntry) -> Result<()> {
        match entry {
            JournalEntry::FileCreated { path } => {
                let p = Path::new(path);
                if p.exists() { std::fs::remove_file(p)?; }
            }
            JournalEntry::DirCreated { path } => {
                let p = Path::new(path);
                // Only remove if empty — safety first
                if p.exists() { let _ = std::fs::remove_dir(p); }
            }
            JournalEntry::FileBackedUp { original, backup } => {
                let src = Path::new(backup);
                let dst = Path::new(original);
                if src.exists() { std::fs::rename(src, dst)?; }
            }
            JournalEntry::ShortcutCreated { path } => {
                let p = Path::new(path);
                if p.exists() { std::fs::remove_file(p)?; }
            }
            // Registry and env var rollback requires Windows APIs — handled separately
            JournalEntry::RegistryWritten { .. }
            | JournalEntry::RegistryKeyCreated { .. }
            | JournalEntry::EnvVarSet { .. }
            | JournalEntry::ServiceInstalled { .. } => {
                log::warn!("Rollback for {:?} requires platform-specific handling", entry);
            }
        }
        Ok(())
    }

    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}