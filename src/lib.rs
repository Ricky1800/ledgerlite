//! ledgerlite — a fast, private, offline CLI that turns bank and
//! credit-card CSV exports into categorized books and a profit-and-loss
//! report.
//!
//! Data never leaves the machine: every command here reads and writes
//! local files only. This crate is split into small, independently
//! testable modules; [`cli`] defines the command-line surface and the
//! `cmd_*` functions in this file are what `main.rs` calls into after
//! parsing arguments.
//!
//! ledgerlite is a bookkeeping tool. It is **not tax advice** — consult a
//! qualified professional (CPA/EA) before making filing decisions.

pub mod cli;
pub mod config;
pub mod daterange;
pub mod dedupe;
pub mod error;
pub mod export;
pub mod importer;
pub mod ledger;
pub mod money;
pub mod overrides;
pub mod profiles;
pub mod report;
pub mod rules;
pub mod transaction;

use crate::error::{LedgerError, Result};
use chrono::NaiveDate;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Resolved locations of every file ledgerlite reads or writes for a
/// given project directory.
pub struct Paths {
    pub config: PathBuf,
    pub rules: PathBuf,
    pub overrides: PathBuf,
    pub data_dir: PathBuf,
    pub ledger: PathBuf,
}

/// Finds `ledgerlite.toml` starting at `start_dir` (searching ancestors),
/// loads it, and resolves every other file path relative to it.
pub fn resolve_paths(start_dir: &Path) -> Result<(config::Config, Paths)> {
    let (cfg, config_path) = config::Config::find_and_load(start_dir)?;
    let base = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let data_dir = cfg.data_dir(&config_path);
    let ledger_path = ledger::ledger_path(&data_dir);
    Ok((
        cfg,
        Paths {
            config: config_path,
            rules: base.join(rules::RULES_FILE_NAME),
            overrides: base.join(overrides::OVERRIDES_FILE_NAME),
            data_dir,
            ledger: ledger_path,
        },
    ))
}

/// `ledgerlite init`
pub fn cmd_init(dir: &Path, force: bool) -> Result<String> {
    let config_path = dir.join(config::CONFIG_FILE_NAME);
    let rules_path = dir.join(rules::RULES_FILE_NAME);

    if !force {
        if config_path.is_file() {
            return Err(LedgerError::AlreadyExists { path: config_path });
        }
        if rules_path.is_file() {
            return Err(LedgerError::AlreadyExists { path: rules_path });
        }
    }

    let cfg = config::Config {
        accounts: vec![
            config::Account {
                name: "Chase Checking".to_string(),
                profile: Some("chase-checking".to_string()),
            },
            config::Account {
                name: "Chase Credit Card".to_string(),
                profile: Some("chase-credit".to_string()),
            },
        ],
        profiles: profiles::built_in_profiles(),
        ..Default::default()
    };
    cfg.save(&config_path)?;

    let rules_file = rules::starter_rules();
    rules_file.save(&rules_path)?;

    Ok(format!(
        "Created {}\nCreated {}\n\nNext steps:\n  1. Edit the [[account]] entries in {} to match your real bank/card names.\n  2. Run: ledgerlite import <file.csv> --account \"Chase Checking\"\n  3. Run: ledgerlite categorize\n  4. Run: ledgerlite report --month 2026-09\n",
        config_path.display(),
        rules_path.display(),
        config::CONFIG_FILE_NAME,
    ))
}

/// `ledgerlite import`
pub fn cmd_import(
    dir: &Path,
    file: &Path,
    account: &str,
    profile_override: Option<&str>,
) -> Result<String> {
    let (cfg, paths) = resolve_paths(dir)?;
    let profile = cfg.resolve_profile(account, profile_override)?;

    let existing = ledger::load(&paths.ledger)?;
    let summary = importer::import_csv(file, account, profile, &existing)?;

    let imported_count = summary.imported.len();
    let mut all = existing;
    all.extend(summary.imported);
    ledger::save(&paths.ledger, &all)?;

    Ok(format!(
        "Imported {imported_count} new transaction(s) from '{}' into account '{account}' \
         ({} row(s) read, {} duplicate(s) skipped).",
        file.display(),
        summary.total_rows,
        summary.duplicate_count,
    ))
}

struct CategorizeChange {
    date: NaiveDate,
    account: String,
    description: String,
    amount: rust_decimal::Decimal,
    old_category: Option<String>,
    new_category: Option<String>,
}

/// `ledgerlite categorize`
pub fn cmd_categorize(dir: &Path, dry_run: bool) -> Result<String> {
    let (_cfg, paths) = resolve_paths(dir)?;
    let rules_file = rules::RulesFile::load(&paths.rules)?;
    let engine = rules::RuleEngine::compile(&rules_file.rules)?;
    let overrides = overrides::Overrides::load_or_default(&paths.overrides)?;

    let mut transactions = ledger::load(&paths.ledger)?;
    let mut changes: Vec<CategorizeChange> = Vec::new();

    for tx in transactions.iter_mut() {
        if let Some(manual_category) = overrides.get(&tx.id) {
            if tx.category.as_deref() != Some(manual_category) {
                changes.push(CategorizeChange {
                    date: tx.date,
                    account: tx.account.clone(),
                    description: tx.description.clone(),
                    amount: tx.amount,
                    old_category: tx.category.clone(),
                    new_category: Some(manual_category.to_string()),
                });
                if !dry_run {
                    tx.category = Some(manual_category.to_string());
                }
            }
            // Rules never overwrite a manual override, matched or not.
            continue;
        }

        if let Some(matched) = engine.categorize(tx) {
            let category_changed = tx.category.as_deref() != Some(matched.category.as_str());
            if category_changed {
                changes.push(CategorizeChange {
                    date: tx.date,
                    account: tx.account.clone(),
                    description: tx.description.clone(),
                    amount: tx.amount,
                    old_category: tx.category.clone(),
                    new_category: Some(matched.category.clone()),
                });
            }
            if !dry_run {
                tx.category = Some(matched.category.clone());
                tx.schedule_c_line = matched.schedule_c_line.clone();
            }
        }
    }

    if !dry_run {
        ledger::save(&paths.ledger, &transactions)?;
    }

    let mut out = String::new();
    if changes.is_empty() {
        out.push_str("No changes — every transaction is already categorized correctly.\n");
    } else {
        for change in &changes {
            out.push_str(&format!(
                "{} {} | {} | {:.2}: {} -> {}\n",
                change.date,
                change.account,
                change.description,
                change.amount,
                change.old_category.as_deref().unwrap_or("Uncategorized"),
                change.new_category.as_deref().unwrap_or("Uncategorized"),
            ));
        }
        if dry_run {
            out.push_str(&format!(
                "\n{} change(s) would be applied (dry run — nothing written).\n",
                changes.len()
            ));
        } else {
            out.push_str(&format!("\n{} change(s) applied.\n", changes.len()));
        }
    }
    Ok(out)
}

/// `ledgerlite report`
#[allow(clippy::too_many_arguments)]
pub fn cmd_report(
    dir: &Path,
    month: Option<&str>,
    year: Option<i32>,
    from: Option<&str>,
    to: Option<&str>,
    format_str: &str,
    schedule_c: bool,
    out: Option<&Path>,
) -> Result<String> {
    let (_cfg, paths) = resolve_paths(dir)?;
    let range = daterange::resolve(month, year, from, to)?;
    let format = report::parse_format(format_str).map_err(LedgerError::UnknownFormat)?;
    let transactions = ledger::load(&paths.ledger)?;
    let rep = report::build_report(
        &transactions,
        range.from,
        range.to,
        &range.label,
        schedule_c,
    );
    let rendered = report::render(&rep, format);

    if let Some(path) = out {
        export::write_to_file(path, &rendered)?;
        Ok(format!("Wrote report to '{}'.", path.display()))
    } else {
        Ok(rendered)
    }
}

/// `ledgerlite export`
pub fn cmd_export(
    dir: &Path,
    format_str: &str,
    output: Option<&Path>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<String> {
    let (_cfg, paths) = resolve_paths(dir)?;
    let format = export::ExportFormat::from_str(format_str)
        .map_err(|_| LedgerError::UnknownFormat(format_str.to_string()))?;
    let transactions = ledger::load(&paths.ledger)?;

    let from_date = from
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| LedgerError::InvalidDate(from.unwrap_or_default().to_string()))?;
    let to_date = to
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| LedgerError::InvalidDate(to.unwrap_or_default().to_string()))?;

    let filtered = export::filter_range(&transactions, from_date, to_date);
    let rendered = export::render(&filtered, format)?;
    let count = filtered.len();

    if let Some(path) = output {
        export::write_to_file(path, &rendered)?;
        Ok(format!(
            "Exported {count} transaction(s) to '{}'.",
            path.display()
        ))
    } else {
        Ok(rendered)
    }
}

/// `ledgerlite attach-receipt <transaction-id> <file>`
pub fn cmd_attach_receipt(dir: &Path, transaction_id: &str, file: &Path) -> Result<String> {
    if !file.exists() {
        return Err(LedgerError::ReceiptFileNotFound {
            path: file.to_path_buf(),
        });
    }

    let (_cfg, paths) = resolve_paths(dir)?;
    let mut transactions = ledger::load(&paths.ledger)?;

    let tx = transactions
        .iter_mut()
        .find(|t| t.id == transaction_id)
        .ok_or_else(|| LedgerError::TransactionNotFound(transaction_id.to_string()))?;

    tx.receipt_path = Some(file.to_path_buf());
    let desc = tx.description.clone();
    let date = tx.date;
    let amount = tx.amount;

    ledger::save(&paths.ledger, &transactions)?;

    Ok(format!(
        "Attached receipt '{}' to transaction '{}' ({}, {}, ${:.2}).\n",
        file.display(),
        transaction_id,
        date,
        desc,
        amount
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_then_import_then_categorize_then_report_end_to_end() {
        let dir = tempdir().unwrap();
        cmd_init(dir.path(), false).unwrap();

        let csv_path = dir.path().join("chase.csv");
        std::fs::write(
            &csv_path,
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,09/05/2026,ADOBE CREATIVE CLOUD,-54.99,DEBIT_CARD,1000.00\n\
             DEBIT,09/06/2026,STARBUCKS STORE #1,-4.50,DEBIT_CARD,995.50\n\
             CREDIT,09/07/2026,CUSTOMER PAYMENT INVOICE 100,500.00,ACH_CREDIT,1495.50\n",
        )
        .unwrap();

        let import_msg = cmd_import(dir.path(), &csv_path, "Chase Checking", None).unwrap();
        assert!(import_msg.contains("Imported 3 new transaction(s)"));

        let categorize_msg = cmd_categorize(dir.path(), false).unwrap();
        assert!(categorize_msg.contains("Software"));

        let report_text = cmd_report(
            dir.path(),
            Some("2026-09"),
            None,
            None,
            None,
            "table",
            true,
            None,
        )
        .unwrap();
        assert!(report_text.contains("Profit & Loss"));
        assert!(report_text.contains("Software"));

        let export_text = cmd_export(dir.path(), "csv", None, None, None).unwrap();
        assert!(export_text.contains("Adobe") || export_text.to_lowercase().contains("adobe"));
    }

    #[test]
    fn categorize_never_overwrites_manual_override() {
        let dir = tempdir().unwrap();
        cmd_init(dir.path(), false).unwrap();

        let csv_path = dir.path().join("chase.csv");
        std::fs::write(
            &csv_path,
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,09/05/2026,ADOBE CREATIVE CLOUD,-54.99,DEBIT_CARD,1000.00\n",
        )
        .unwrap();
        cmd_import(dir.path(), &csv_path, "Chase Checking", None).unwrap();

        let (_, paths) = resolve_paths(dir.path()).unwrap();
        let transactions = ledger::load(&paths.ledger).unwrap();
        let tx_id = transactions[0].id.clone();

        let mut overrides = overrides::Overrides::load_or_default(&paths.overrides).unwrap();
        overrides.set(tx_id.clone(), "Owner Draw");
        overrides.save(&paths.overrides).unwrap();

        cmd_categorize(dir.path(), false).unwrap();

        let transactions = ledger::load(&paths.ledger).unwrap();
        let tx = transactions.iter().find(|t| t.id == tx_id).unwrap();
        assert_eq!(tx.category.as_deref(), Some("Owner Draw"));
    }

    #[test]
    fn init_refuses_to_overwrite_without_force() {
        let dir = tempdir().unwrap();
        cmd_init(dir.path(), false).unwrap();
        let err = cmd_init(dir.path(), false).unwrap_err();
        assert!(matches!(err, LedgerError::AlreadyExists { .. }));
    }

    #[test]
    fn import_without_config_gives_no_config_error() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("chase.csv");
        std::fs::write(&csv_path, "Date,Description,Amount\n2026-01-01,X,-1.00\n").unwrap();
        let err = cmd_import(dir.path(), &csv_path, "Chase Checking", None).unwrap_err();
        assert!(matches!(err, LedgerError::NoConfig));
    }
}
