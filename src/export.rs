//! `ledgerlite export` — dumping transactions to CSV or JSON for handing
//! off to an accountant or another tool.

use crate::error::{LedgerError, Result};
use crate::transaction::Transaction;
use chrono::NaiveDate;
use serde::Serialize;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

impl FromStr for ExportFormat {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "csv" => Ok(ExportFormat::Csv),
            "json" => Ok(ExportFormat::Json),
            _ => Err(()),
        }
    }
}

#[derive(Serialize)]
struct ExportRow<'a> {
    date: String,
    account: &'a str,
    description: &'a str,
    amount: String,
    category: &'a str,
    schedule_c_line: &'a str,
    source_file: &'a str,
}

pub fn filter_range(
    transactions: &[Transaction],
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<&Transaction> {
    transactions
        .iter()
        .filter(|t| {
            from.map(|f| t.date >= f).unwrap_or(true) && to.map(|to| t.date <= to).unwrap_or(true)
        })
        .collect()
}

pub fn render_csv(transactions: &[&Transaction]) -> Result<String> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    for tx in transactions {
        let row = ExportRow {
            date: tx.date.to_string(),
            account: &tx.account,
            description: &tx.description,
            amount: tx.amount.to_string(),
            category: tx.category.as_deref().unwrap_or(""),
            schedule_c_line: tx.schedule_c_line.as_deref().unwrap_or(""),
            source_file: &tx.source_file,
        };
        wtr.serialize(row).map_err(|source| LedgerError::Csv {
            path: "<export>".into(),
            source: Box::new(source),
        })?;
    }
    let bytes = wtr.into_inner().unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub fn render_json(transactions: &[&Transaction]) -> Result<String> {
    serde_json::to_string_pretty(transactions).map_err(|source| LedgerError::Json {
        path: "<export>".into(),
        source,
    })
}

pub fn render(transactions: &[&Transaction], format: ExportFormat) -> Result<String> {
    match format {
        ExportFormat::Csv => render_csv(transactions),
        ExportFormat::Json => render_json(transactions),
    }
}

pub fn write_to_file(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).map_err(|source| LedgerError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tx(date: &str, amount: rust_decimal::Decimal) -> Transaction {
        Transaction {
            id: date.to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            account: "Chase Checking".into(),
            description: "test".into(),
            raw_description: "test".into(),
            amount,
            category: Some("Software".into()),
            schedule_c_line: None,
            source_file: "f.csv".into(),
            occurrence: 0,
        }
    }

    #[test]
    fn filters_by_range() {
        let txs = vec![
            tx("2026-01-01", dec!(-1)),
            tx("2026-02-01", dec!(-2)),
            tx("2026-03-01", dec!(-3)),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let filtered = filter_range(&txs, Some(from), None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn csv_export_contains_header_and_rows() {
        let txs = [tx("2026-01-01", dec!(-5.00))];
        let refs: Vec<&Transaction> = txs.iter().collect();
        let csv_text = render_csv(&refs).unwrap();
        assert!(csv_text.contains("date,account,description,amount"));
        assert!(csv_text.contains("2026-01-01"));
    }

    #[test]
    fn json_export_round_trips_shape() {
        let txs = [tx("2026-01-01", dec!(-5.00))];
        let refs: Vec<&Transaction> = txs.iter().collect();
        let json_text = render_json(&refs).unwrap();
        assert!(json_text.contains("\"amount\""));
    }
}
