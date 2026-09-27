//! Manual category overrides.
//!
//! `overrides.toml` maps a transaction's stable id (see
//! [`crate::transaction::dedupe_key`]) to a category name that the user
//! chose by hand. `ledgerlite categorize` always checks this file first
//! and never replaces a category it finds there — rules only ever fill
//! in transactions that have no override.

use crate::error::{LedgerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const OVERRIDES_FILE_NAME: &str = "overrides.toml";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Overrides {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
}

impl Overrides {
    /// Loads `overrides.toml` if it exists; an absent file is treated as
    /// "no overrides" rather than an error, since it's optional.
    pub fn load_or_default(path: &Path) -> Result<Overrides> {
        if !path.is_file() {
            return Ok(Overrides::default());
        }
        let text = std::fs::read_to_string(path).map_err(|source| LedgerError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| LedgerError::TomlParse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|source| LedgerError::TomlSerialize {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        std::fs::write(path, text).map_err(|source| LedgerError::WriteFile {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn get(&self, transaction_id: &str) -> Option<&str> {
        self.overrides.get(transaction_id).map(|s| s.as_str())
    }

    pub fn set(&mut self, transaction_id: impl Into<String>, category: impl Into<String>) {
        self.overrides
            .insert(transaction_id.into(), category.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_file_yields_empty_overrides() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(OVERRIDES_FILE_NAME);
        let overrides = Overrides::load_or_default(&path).unwrap();
        assert!(overrides.overrides.is_empty());
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(OVERRIDES_FILE_NAME);
        let mut overrides = Overrides::default();
        overrides.set("2026-01-05|Chase Checking|-4.50|starbucks|0", "Meals");
        overrides.save(&path).unwrap();

        let loaded = Overrides::load_or_default(&path).unwrap();
        assert_eq!(
            loaded.get("2026-01-05|Chase Checking|-4.50|starbucks|0"),
            Some("Meals")
        );
    }
}
