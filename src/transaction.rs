//! The core `Transaction` record and the normalization helpers used to
//! build a stable dedupe key for it.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A single normalized ledger entry.
///
/// Sign convention: `amount` is **positive for money coming in** (sales,
/// refunds received) and **negative for money going out** (expenses,
/// fees). This is the convention Chase, Bank of America, and Capital One
/// already export in; Amex exports the opposite way and is flipped on
/// import (see [`crate::config::SignConvention`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// Stable identifier used for deduping and for manual category
    /// overrides. See [`dedupe_key`].
    pub id: String,
    pub date: NaiveDate,
    pub account: String,
    /// Description as it will be shown in reports (currently identical to
    /// `raw_description`; kept separate so future cleanup rules have
    /// somewhere to write to without losing the original).
    pub description: String,
    /// Description exactly as it appeared in the source CSV.
    pub raw_description: String,
    pub amount: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_c_line: Option<String>,
    /// File this row was imported from, for traceability.
    pub source_file: String,
    /// 0-indexed position of this transaction within its
    /// (date, account, amount, normalized description) group. Two
    /// genuinely identical same-day purchases get occurrence 0 and 1;
    /// re-importing an overlapping export reproduces the same
    /// occurrences and is therefore recognized as a duplicate.
    pub occurrence: u32,
    /// Optional path to a receipt file (e.g. PDF or image) on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_path: Option<std::path::PathBuf>,
}

/// Collapses internal whitespace and lowercases a description so that
/// trivial formatting differences between overlapping exports of the same
/// transaction don't defeat deduping.
pub fn normalize_description(desc: &str) -> String {
    desc.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The part of the dedupe key that identifies a *group* of
/// same-day/same-amount/same-description transactions, before the
/// occurrence index is appended.
pub fn group_key(
    date: NaiveDate,
    account: &str,
    amount: Decimal,
    normalized_description: &str,
) -> String {
    format!("{date}|{account}|{amount}|{normalized_description}")
}

/// Builds the full, stable dedupe key/id for a transaction.
pub fn dedupe_key(
    date: NaiveDate,
    account: &str,
    amount: Decimal,
    normalized_description: &str,
    occurrence: u32,
) -> String {
    format!(
        "{}|{occurrence}",
        group_key(date, account, amount, normalized_description)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn normalize_collapses_whitespace_and_lowercases() {
        assert_eq!(
            normalize_description("  STARBUCKS   Store #123 "),
            "starbucks store #123"
        );
    }

    #[test]
    fn dedupe_key_is_stable_for_identical_inputs() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let a = dedupe_key(date, "Chase Checking", dec!(-4.50), "starbucks", 0);
        let b = dedupe_key(date, "Chase Checking", dec!(-4.50), "starbucks", 0);
        assert_eq!(a, b);
    }

    #[test]
    fn dedupe_key_differs_by_occurrence() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let a = dedupe_key(date, "Chase Checking", dec!(-4.50), "starbucks", 0);
        let b = dedupe_key(date, "Chase Checking", dec!(-4.50), "starbucks", 1);
        assert_ne!(a, b);
    }
}
