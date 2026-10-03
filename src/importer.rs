//! CSV import: reads a bank/credit-card export according to an
//! [`ImportProfile`] column mapping, normalizes rows into [`Transaction`]s,
//! and dedupes against the existing ledger.

use crate::config::{ImportProfile, SignConvention};
use crate::dedupe::OccurrenceTracker;
use crate::error::{LedgerError, Result};
use crate::money::parse_money;
use crate::transaction::Transaction;
use chrono::NaiveDate;
use std::path::Path;

#[derive(Debug)]
pub struct ImportSummary {
    pub imported: Vec<Transaction>,
    pub duplicate_count: usize,
    pub total_rows: usize,
}

fn column_index(headers: &csv::StringRecord, path: &Path, column: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h == column)
        .ok_or_else(|| LedgerError::MissingColumn {
            path: path.to_path_buf(),
            column: column.to_string(),
            available: headers.iter().collect::<Vec<_>>().join(", "),
        })
}

/// Imports `path` for `account` using `profile`, deduping against
/// `existing`. Does not mutate `existing`; the caller decides what to do
/// with the returned [`ImportSummary`] (typically: append `imported` to
/// the ledger and save).
pub fn import_csv(
    path: &Path,
    account: &str,
    profile: &ImportProfile,
    existing: &[Transaction],
) -> Result<ImportSummary> {
    profile.validate("(selected profile)")?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|source| LedgerError::Csv {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;

    let headers = reader
        .headers()
        .map_err(|source| LedgerError::Csv {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?
        .clone();
    if headers.is_empty() {
        return Err(LedgerError::MissingHeader {
            path: path.to_path_buf(),
        });
    }

    let date_idx = column_index(&headers, path, &profile.date_column)?;
    let desc_idx = column_index(&headers, path, &profile.description_column)?;
    let amount_idx = match &profile.amount_column {
        Some(c) => Some(column_index(&headers, path, c)?),
        None => None,
    };
    let debit_idx = match &profile.debit_column {
        Some(c) => Some(column_index(&headers, path, c)?),
        None => None,
    };
    let credit_idx = match &profile.credit_column {
        Some(c) => Some(column_index(&headers, path, c)?),
        None => None,
    };

    let mut tracker = OccurrenceTracker::from_existing(existing);
    let source_file = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let mut imported = Vec::new();
    let mut duplicate_count = 0usize;
    let mut total_rows = 0usize;

    for (record_idx, record) in reader.records().enumerate() {
        // CSV row numbers as a human would count them: header is row 1,
        // first data row is row 2.
        let row = record_idx + 2;
        let record = record.map_err(|source| LedgerError::Csv {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        total_rows += 1;

        if record.len() != headers.len() {
            return Err(LedgerError::RowShapeMismatch {
                path: path.to_path_buf(),
                row,
                found: record.len(),
                expected: headers.len(),
            });
        }

        let date_raw = record.get(date_idx).unwrap_or("").trim();
        let date = NaiveDate::parse_from_str(date_raw, &profile.date_format).map_err(|_| {
            LedgerError::DateParse {
                path: path.to_path_buf(),
                row,
                column: profile.date_column.clone(),
                value: date_raw.to_string(),
                format: profile.date_format.clone(),
            }
        })?;

        let raw_description = record.get(desc_idx).unwrap_or("").trim().to_string();

        let mut amount = if let Some(idx) = amount_idx {
            let raw = record.get(idx).unwrap_or("").trim();
            parse_money(raw).map_err(|reason| LedgerError::AmountParse {
                path: path.to_path_buf(),
                row,
                column: profile.amount_column.clone().unwrap_or_default(),
                value: raw.to_string(),
                reason,
            })?
        } else {
            let debit_raw = debit_idx.and_then(|i| record.get(i)).unwrap_or("").trim();
            let credit_raw = credit_idx.and_then(|i| record.get(i)).unwrap_or("").trim();
            let debit = if debit_raw.is_empty() {
                rust_decimal::Decimal::ZERO
            } else {
                parse_money(debit_raw).map_err(|reason| LedgerError::AmountParse {
                    path: path.to_path_buf(),
                    row,
                    column: profile.debit_column.clone().unwrap_or_default(),
                    value: debit_raw.to_string(),
                    reason,
                })?
            };
            let credit = if credit_raw.is_empty() {
                rust_decimal::Decimal::ZERO
            } else {
                parse_money(credit_raw).map_err(|reason| LedgerError::AmountParse {
                    path: path.to_path_buf(),
                    row,
                    column: profile.credit_column.clone().unwrap_or_default(),
                    value: credit_raw.to_string(),
                    reason,
                })?
            };
            credit - debit.abs()
        };

        if profile.sign == SignConvention::Negated {
            amount = -amount;
        }

        let (id, occurrence, is_duplicate) = tracker.next(date, account, amount, &raw_description);
        if is_duplicate {
            duplicate_count += 1;
            continue;
        }

        imported.push(Transaction {
            id,
            date,
            account: account.to_string(),
            description: raw_description.clone(),
            raw_description,
            amount,
            category: None,
            schedule_c_line: None,
            source_file: source_file.clone(),
            occurrence,
            receipt_path: None,
        });
    }

    Ok(ImportSummary {
        imported,
        duplicate_count,
        total_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ImportProfile;
    use rust_decimal_macros::dec;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_csv(contents: &str) -> NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        write!(file, "{contents}").unwrap();
        file
    }

    fn chase_checking_profile() -> ImportProfile {
        crate::profiles::built_in_profiles()
            .remove("chase-checking")
            .unwrap()
    }

    #[test]
    fn imports_basic_rows() {
        let file = write_csv(
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,01/05/2026,STARBUCKS STORE #1,-4.50,DEBIT_CARD,1000.00\n\
             CREDIT,01/06/2026,PAYROLL DEPOSIT,2000.00,ACH_CREDIT,3000.00\n",
        );
        let profile = chase_checking_profile();
        let summary = import_csv(file.path(), "Chase Checking", &profile, &[]).unwrap();
        assert_eq!(summary.total_rows, 2);
        assert_eq!(summary.duplicate_count, 0);
        assert_eq!(summary.imported.len(), 2);
        assert_eq!(summary.imported[0].amount, dec!(-4.50));
        assert_eq!(summary.imported[1].amount, dec!(2000.00));
    }

    #[test]
    fn dedupes_against_existing_ledger_on_reimport() {
        let file = write_csv(
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,01/05/2026,STARBUCKS STORE #1,-4.50,DEBIT_CARD,1000.00\n",
        );
        let profile = chase_checking_profile();
        let first = import_csv(file.path(), "Chase Checking", &profile, &[]).unwrap();
        assert_eq!(first.imported.len(), 1);

        let second = import_csv(file.path(), "Chase Checking", &profile, &first.imported).unwrap();
        assert_eq!(second.imported.len(), 0);
        assert_eq!(second.duplicate_count, 1);
    }

    #[test]
    fn keeps_genuine_same_day_duplicates_distinct() {
        let file = write_csv(
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,01/05/2026,STARBUCKS STORE #1,-4.50,DEBIT_CARD,1000.00\n\
             DEBIT,01/05/2026,STARBUCKS STORE #1,-4.50,DEBIT_CARD,995.50\n",
        );
        let profile = chase_checking_profile();
        let summary = import_csv(file.path(), "Chase Checking", &profile, &[]).unwrap();
        assert_eq!(summary.imported.len(), 2);
        assert_ne!(summary.imported[0].id, summary.imported[1].id);
    }

    #[test]
    fn amex_sign_convention_is_negated() {
        let file = write_csv("Date,Description,Amount\n01/05/2026,OFFICE SUPPLIES,54.00\n");
        let profile = crate::profiles::built_in_profiles().remove("amex").unwrap();
        let summary = import_csv(file.path(), "Amex", &profile, &[]).unwrap();
        // Amex reports charges as positive; ledgerlite convention is
        // negative for money out, so it must be flipped.
        assert_eq!(summary.imported[0].amount, dec!(-54.00));
    }

    #[test]
    fn capital_one_combines_debit_and_credit_columns() {
        let file = write_csv(
            "Transaction Date,Posted Date,Card No.,Description,Category,Debit,Credit\n\
             2026-01-05,2026-01-06,1234,OFFICE DEPOT,Business,54.00,\n\
             2026-01-07,2026-01-08,1234,REFUND,Business,,20.00\n",
        );
        let profile = crate::profiles::built_in_profiles()
            .remove("capital-one")
            .unwrap();
        let summary = import_csv(file.path(), "Capital One", &profile, &[]).unwrap();
        assert_eq!(summary.imported[0].amount, dec!(-54.00));
        assert_eq!(summary.imported[1].amount, dec!(20.00));
    }

    #[test]
    fn missing_column_produces_clear_error() {
        let file = write_csv("Date,Description\n01/05/2026,X\n");
        let profile = chase_checking_profile();
        let err = import_csv(file.path(), "Chase Checking", &profile, &[]).unwrap_err();
        match err {
            LedgerError::MissingColumn { column, .. } => assert_eq!(column, "Posting Date"),
            other => panic!("expected MissingColumn, got {other:?}"),
        }
    }

    #[test]
    fn bad_date_produces_row_and_column_context() {
        let file = write_csv(
            "Details,Posting Date,Description,Amount,Type,Balance\n\
             DEBIT,not-a-date,STARBUCKS,-4.50,DEBIT_CARD,1000.00\n",
        );
        let profile = chase_checking_profile();
        let err = import_csv(file.path(), "Chase Checking", &profile, &[]).unwrap_err();
        match err {
            LedgerError::DateParse { row, column, .. } => {
                assert_eq!(row, 2);
                assert_eq!(column, "Posting Date");
            }
            other => panic!("expected DateParse, got {other:?}"),
        }
    }
}
