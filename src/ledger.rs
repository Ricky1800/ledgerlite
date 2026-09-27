//! Reading and writing the local ledger data file (JSON Lines — one
//! transaction per line, so it stays diff-friendly and can be tailed/grepped
//! by hand if needed).

use crate::error::{LedgerError, Result};
use crate::transaction::Transaction;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const LEDGER_FILE_NAME: &str = "ledger.jsonl";

pub fn ledger_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LEDGER_FILE_NAME)
}

/// Loads every transaction from the ledger file. A missing file is not an
/// error — it just means nothing has been imported yet.
pub fn load(path: &Path) -> Result<Vec<Transaction>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(path).map_err(|source| LedgerError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut transactions = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| LedgerError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let tx: Transaction =
            serde_json::from_str(&line).map_err(|source| LedgerError::LedgerLineParse {
                path: path.to_path_buf(),
                line: idx + 1,
                source,
            })?;
        transactions.push(tx);
    }
    Ok(transactions)
}

/// Writes the full transaction set back out, sorted by date (then id) so
/// the file stays stable across runs.
pub fn save(path: &Path, transactions: &[Transaction]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| LedgerError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }

    let mut sorted: Vec<&Transaction> = transactions.iter().collect();
    sorted.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));

    let mut file = std::fs::File::create(path).map_err(|source| LedgerError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;
    for tx in sorted {
        let line = serde_json::to_string(tx).map_err(|source| LedgerError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        writeln!(file, "{line}").map_err(|source| LedgerError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    fn sample_tx(id: &str, date: NaiveDate) -> Transaction {
        Transaction {
            id: id.to_string(),
            date,
            account: "Chase Checking".into(),
            description: "Starbucks".into(),
            raw_description: "STARBUCKS".into(),
            amount: dec!(-4.50),
            category: None,
            schedule_c_line: None,
            source_file: "a.csv".into(),
            occurrence: 0,
        }
    }

    #[test]
    fn loading_missing_file_yields_empty_vec() {
        let dir = tempdir().unwrap();
        let path = ledger_path(dir.path());
        assert_eq!(load(&path).unwrap().len(), 0);
    }

    #[test]
    fn round_trips_transactions() {
        let dir = tempdir().unwrap();
        let path = ledger_path(dir.path());
        let txs = vec![
            sample_tx("b", NaiveDate::from_ymd_opt(2026, 1, 6).unwrap()),
            sample_tx("a", NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
        ];
        save(&path, &txs).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        // Sorted by date first.
        assert_eq!(loaded[0].id, "a");
        assert_eq!(loaded[1].id, "b");
    }

    #[test]
    fn rejects_corrupt_line_with_line_number() {
        let dir = tempdir().unwrap();
        let path = ledger_path(dir.path());
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(&path, "{\"id\":\"ok\"}\nnot json\n").unwrap();
        let err = load(&path).unwrap_err();
        match err {
            LedgerError::LedgerLineParse { line, .. } => assert_eq!(line, 1),
            other => panic!("expected LedgerLineParse, got {other:?}"),
        }
    }
}
