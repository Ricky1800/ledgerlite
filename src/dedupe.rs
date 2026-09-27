//! Deduping logic shared by the importer.
//!
//! The dedupe key is `date + amount + normalized description + account +
//! occurrence index`. The occurrence index is assigned by *position
//! within the file currently being imported* — the first row matching a
//! given (date, account, amount, description) group gets occurrence 0,
//! the second gets 1, and so on — which is what lets two genuinely
//! identical same-day purchases in one export both be kept as distinct
//! transactions.
//!
//! Re-importing an overlapping export reproduces the exact same
//! occurrence sequence for the exact same rows (same file, same order),
//! so the ids it produces already exist in the ledger and are recognized
//! as duplicates. Known limitation: if a genuinely new same-day duplicate
//! transaction shows up in a *separate*, non-overlapping later import for
//! a group that already has entries in the ledger, it can collide with
//! an existing occurrence and be mistaken for a duplicate — see
//! `ROADMAP_ISSUES.md`/README for how to work around this (import the
//! full overlapping range in one file when possible).
use crate::transaction::{dedupe_key, group_key, normalize_description, Transaction};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

/// Tracks, per "group" (date/account/amount/description), how many rows
/// have been assigned an occurrence index *within the current import
/// call*, and separately holds the set of ids already present in the
/// ledger so newly computed ids can be checked against it.
pub struct OccurrenceTracker {
    /// Per-file counter; always starts empty for a new import call.
    counts: HashMap<String, u32>,
    /// Ids already present in the ledger before this import started.
    existing_ids: HashSet<String>,
}

impl OccurrenceTracker {
    /// Builds the tracker from the transactions already present in the
    /// ledger. The per-file occurrence counter always starts fresh —
    /// only the id set carries over, for duplicate detection.
    pub fn from_existing(existing: &[Transaction]) -> OccurrenceTracker {
        let existing_ids = existing.iter().map(|tx| tx.id.clone()).collect();
        OccurrenceTracker {
            counts: HashMap::new(),
            existing_ids,
        }
    }

    /// Assigns the next occurrence index for a row and returns the full
    /// dedupe id, the occurrence index itself, and whether that id is
    /// already present in the ledger (i.e. this row is a duplicate that
    /// should be skipped).
    pub fn next(
        &mut self,
        date: NaiveDate,
        account: &str,
        amount: Decimal,
        raw_description: &str,
    ) -> (String, u32, bool) {
        let normalized = normalize_description(raw_description);
        let key = group_key(date, account, amount, &normalized);
        let occurrence = *self.counts.get(&key).unwrap_or(&0);
        self.counts.insert(key, occurrence + 1);

        let id = dedupe_key(date, account, amount, &normalized, occurrence);
        let is_duplicate = self.existing_ids.contains(&id);
        (id, occurrence, is_duplicate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn distinct_same_day_purchases_get_different_occurrences() {
        let mut tracker = OccurrenceTracker::from_existing(&[]);
        let (id1, occ1, dup1) =
            tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        let (id2, occ2, dup2) =
            tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        assert_ne!(id1, id2);
        assert_eq!(occ1, 0);
        assert_eq!(occ2, 1);
        assert!(!dup1);
        assert!(!dup2);
    }

    #[test]
    fn reimporting_the_same_rows_is_recognized_as_duplicate() {
        // First "import": two identical rows.
        let mut tracker = OccurrenceTracker::from_existing(&[]);
        let (id1, _, _) = tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        let (id2, _, _) = tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");

        let existing = vec![
            Transaction {
                id: id1.clone(),
                date: d(2026, 1, 5),
                account: "Chase Checking".into(),
                description: "Starbucks".into(),
                raw_description: "Starbucks".into(),
                amount: dec!(-4.50),
                category: None,
                schedule_c_line: None,
                source_file: "a.csv".into(),
                occurrence: 0,
            },
            Transaction {
                id: id2.clone(),
                date: d(2026, 1, 5),
                account: "Chase Checking".into(),
                description: "Starbucks".into(),
                raw_description: "Starbucks".into(),
                amount: dec!(-4.50),
                category: None,
                schedule_c_line: None,
                source_file: "a.csv".into(),
                occurrence: 1,
            },
        ];

        // Second "import" of an overlapping export containing the same
        // two rows plus one genuinely new one.
        let mut tracker2 = OccurrenceTracker::from_existing(&existing);
        let (dup_id1, _, is_dup1) =
            tracker2.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        let (dup_id2, _, is_dup2) =
            tracker2.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        let (new_id, _, is_dup3) =
            tracker2.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");

        assert_eq!(dup_id1, id1);
        assert!(is_dup1);
        assert_eq!(dup_id2, id2);
        assert!(is_dup2);
        assert!(!is_dup3);
        assert_ne!(new_id, id1);
        assert_ne!(new_id, id2);
    }

    #[test]
    fn different_amounts_never_collide() {
        let mut tracker = OccurrenceTracker::from_existing(&[]);
        let (id1, _, _) = tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-4.50), "Starbucks");
        let (id2, _, _) = tracker.next(d(2026, 1, 5), "Chase Checking", dec!(-5.00), "Starbucks");
        assert_ne!(id1, id2);
    }
}
