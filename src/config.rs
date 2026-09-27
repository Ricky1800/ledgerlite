//! `ledgerlite.toml` — accounts and import profile definitions.

use crate::error::{LedgerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "ledgerlite.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SignConvention {
    /// The file's amount column already uses ledgerlite's convention:
    /// negative = money out, positive = money in.
    #[default]
    AsIs,
    /// The file's amount column is inverted relative to ledgerlite's
    /// convention (e.g. Amex reports charges as positive numbers).
    Negated,
}

/// Column mapping for one CSV export format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportProfile {
    pub date_column: String,
    pub date_format: String,
    pub description_column: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debit_column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_column: Option<String>,
    #[serde(default)]
    pub sign: SignConvention,
}

impl ImportProfile {
    pub fn validate(&self, profile_name: &str) -> Result<()> {
        let has_amount = self.amount_column.is_some();
        let has_debit_credit = self.debit_column.is_some() || self.credit_column.is_some();
        if !has_amount && !has_debit_credit {
            return Err(LedgerError::ProfileMissingAmount(profile_name.to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

fn default_data_dir() -> String {
    "data".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct General {
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

impl Default for General {
    fn default() -> Self {
        General {
            data_dir: default_data_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default, rename = "account")]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub profiles: HashMap<String, ImportProfile>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).map_err(|source| LedgerError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| LedgerError::TomlParse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    /// Looks for `ledgerlite.toml` in `start_dir` and its ancestors.
    pub fn find_and_load(start_dir: &Path) -> Result<(Config, PathBuf)> {
        let mut dir = Some(start_dir.to_path_buf());
        while let Some(d) = dir {
            let candidate = d.join(CONFIG_FILE_NAME);
            if candidate.is_file() {
                let cfg = Config::load(&candidate)?;
                return Ok((cfg, candidate));
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
        Err(LedgerError::NoConfig)
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

    /// Resolves the import profile to use for an `import` invocation:
    /// an explicit `--profile` flag wins, otherwise the account's
    /// configured default profile is used.
    pub fn resolve_profile(
        &self,
        account: &str,
        explicit_profile: Option<&str>,
    ) -> Result<&ImportProfile> {
        let profile_name = if let Some(p) = explicit_profile {
            p.to_string()
        } else {
            let account_entry = self.accounts.iter().find(|a| a.name == account);
            match account_entry.and_then(|a| a.profile.as_deref()) {
                Some(p) => p.to_string(),
                None => return Err(LedgerError::NoProfileForAccount(account.to_string())),
            }
        };

        self.profiles
            .get(&profile_name)
            .ok_or(LedgerError::UnknownProfile(profile_name))
    }

    pub fn data_dir(&self, config_path: &Path) -> PathBuf {
        let base = config_path.parent().unwrap_or_else(|| Path::new("."));
        base.join(&self.general.data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_profile_prefers_explicit_flag() {
        let mut cfg = Config::default();
        cfg.accounts.push(Account {
            name: "Chase Checking".to_string(),
            profile: Some("chase-checking".to_string()),
        });
        cfg.profiles.insert(
            "generic".to_string(),
            ImportProfile {
                date_column: "Date".into(),
                date_format: "%Y-%m-%d".into(),
                description_column: "Description".into(),
                amount_column: Some("Amount".into()),
                debit_column: None,
                credit_column: None,
                sign: SignConvention::AsIs,
            },
        );
        let resolved = cfg
            .resolve_profile("Chase Checking", Some("generic"))
            .unwrap();
        assert_eq!(resolved.date_column, "Date");
    }

    #[test]
    fn resolve_profile_falls_back_to_account_default() {
        let mut cfg = Config::default();
        cfg.accounts.push(Account {
            name: "Chase Checking".to_string(),
            profile: Some("chase-checking".to_string()),
        });
        cfg.profiles.insert(
            "chase-checking".to_string(),
            ImportProfile {
                date_column: "Posting Date".into(),
                date_format: "%m/%d/%Y".into(),
                description_column: "Description".into(),
                amount_column: Some("Amount".into()),
                debit_column: None,
                credit_column: None,
                sign: SignConvention::AsIs,
            },
        );
        let resolved = cfg.resolve_profile("Chase Checking", None).unwrap();
        assert_eq!(resolved.date_column, "Posting Date");
    }

    #[test]
    fn resolve_profile_errors_on_unknown_account_without_profile() {
        let cfg = Config::default();
        let err = cfg.resolve_profile("Unknown Bank", None).unwrap_err();
        assert!(matches!(err, LedgerError::NoProfileForAccount(_)));
    }

    #[test]
    fn validate_rejects_profile_without_amount_source() {
        let profile = ImportProfile {
            date_column: "Date".into(),
            date_format: "%Y-%m-%d".into(),
            description_column: "Description".into(),
            amount_column: None,
            debit_column: None,
            credit_column: None,
            sign: SignConvention::AsIs,
        };
        assert!(profile.validate("broken").is_err());
    }
}
