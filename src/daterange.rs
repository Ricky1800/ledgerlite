//! Resolves the `--month` / `--year` / `--from`/`--to` family of CLI flags
//! into a single inclusive date range plus a human-readable label.

use crate::error::{LedgerError, Result};
use chrono::{Datelike, NaiveDate};

pub struct ResolvedRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub label: String,
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
pub fn resolve(
    month: Option<&str>,
    year: Option<i32>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<ResolvedRange> {
    let specified = [
        month.is_some(),
        year.is_some(),
        from.is_some() || to.is_some(),
    ];
    if specified.iter().filter(|s| **s).count() > 1 {
        return Err(LedgerError::ConflictingDateRange);
    }

    if let Some(month_str) = month {
        let parts: Vec<&str> = month_str.split('-').collect();
        if parts.len() != 2 {
            return Err(LedgerError::InvalidMonth(month_str.to_string()));
        }
        let year: i32 = parts[0]
            .parse()
            .map_err(|_| LedgerError::InvalidMonth(month_str.to_string()))?;
        let month: u32 = parts[1]
            .parse()
            .map_err(|_| LedgerError::InvalidMonth(month_str.to_string()))?;
        let from = NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or_else(|| LedgerError::InvalidMonth(month_str.to_string()))?;
        let to = last_day_of_month(year, month);
        return Ok(ResolvedRange {
            from,
            to,
            label: format!("{year:04}-{month:02}"),
        });
    }

    if let Some(year) = year {
        let from = NaiveDate::from_ymd_opt(year, 1, 1)
            .ok_or_else(|| LedgerError::InvalidYear(year.to_string()))?;
        let to = NaiveDate::from_ymd_opt(year, 12, 31)
            .ok_or_else(|| LedgerError::InvalidYear(year.to_string()))?;
        return Ok(ResolvedRange {
            from,
            to,
            label: format!("{year:04}"),
        });
    }

    if from.is_some() || to.is_some() {
        let from_str = from.ok_or(LedgerError::MissingDateRange)?;
        let to_str = to.ok_or(LedgerError::MissingDateRange)?;
        let from_date = NaiveDate::parse_from_str(from_str, "%Y-%m-%d")
            .map_err(|_| LedgerError::InvalidDate(from_str.to_string()))?;
        let to_date = NaiveDate::parse_from_str(to_str, "%Y-%m-%d")
            .map_err(|_| LedgerError::InvalidDate(to_str.to_string()))?;
        if from_date > to_date {
            return Err(LedgerError::InvalidDateRange(format!(
                "--from {from_str} is after --to {to_str}"
            )));
        }
        return Ok(ResolvedRange {
            from: from_date,
            to: to_date,
            label: format!("{from_str} to {to_str}"),
        });
    }

    Err(LedgerError::MissingDateRange)
}

/// Convenience used by the report command to double check a year is
/// plausible (chrono itself will happily construct far-future dates).
pub fn sanity_check_year(date: NaiveDate) -> bool {
    (1900..=2200).contains(&date.year())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_month() {
        let r = resolve(Some("2026-09"), None, None, None).unwrap();
        assert_eq!(r.from, NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        assert_eq!(r.to, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
    }

    #[test]
    fn resolves_month_handles_december_rollover() {
        let r = resolve(Some("2026-12"), None, None, None).unwrap();
        assert_eq!(r.to, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn resolves_year() {
        let r = resolve(None, Some(2026), None, None).unwrap();
        assert_eq!(r.from, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(r.to, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn resolves_from_to() {
        let r = resolve(None, None, Some("2026-01-15"), Some("2026-02-10")).unwrap();
        assert_eq!(r.from, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(r.to, NaiveDate::from_ymd_opt(2026, 2, 10).unwrap());
    }

    #[test]
    fn rejects_conflicting_flags() {
        assert!(resolve(Some("2026-09"), Some(2026), None, None).is_err());
    }

    #[test]
    fn rejects_nothing_given() {
        assert!(resolve(None, None, None, None).is_err());
    }

    #[test]
    fn rejects_from_after_to() {
        assert!(resolve(None, None, Some("2026-02-10"), Some("2026-01-15")).is_err());
    }

    #[test]
    fn rejects_bad_month_format() {
        assert!(resolve(Some("2026/09"), None, None, None).is_err());
    }
}
