//! Profit & loss report generation and rendering.

use crate::transaction::Transaction;
use chrono::NaiveDate;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::str::FromStr;

pub const UNCATEGORIZED_LABEL: &str = "Uncategorized";

#[derive(Debug, Clone, Serialize)]
pub struct CategoryLine {
    pub category: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct PnlReport {
    pub label: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub income: Vec<CategoryLine>,
    pub expenses: Vec<CategoryLine>,
    pub total_income: Decimal,
    pub total_expenses: Decimal,
    pub net: Decimal,
    pub uncategorized: Vec<Transaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_c: Option<Vec<CategoryLine>>,
}

fn to_sorted_lines(map: BTreeMap<String, Decimal>) -> Vec<CategoryLine> {
    let mut lines: Vec<CategoryLine> = map
        .into_iter()
        .map(|(category, amount)| CategoryLine { category, amount })
        .collect();
    lines.sort_by(|a, b| {
        b.amount
            .abs()
            .cmp(&a.amount.abs())
            .then_with(|| a.category.cmp(&b.category))
    });
    lines
}

/// Builds a P&L report from already-categorized transactions in
/// `[from, to]` (inclusive on both ends).
pub fn build_report(
    transactions: &[Transaction],
    from: NaiveDate,
    to: NaiveDate,
    label: &str,
    include_schedule_c: bool,
) -> PnlReport {
    let in_range: Vec<&Transaction> = transactions
        .iter()
        .filter(|t| t.date >= from && t.date <= to)
        .collect();

    let mut income_map: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut expense_map: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut schedule_c_map: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut uncategorized = Vec::new();
    let mut total_income = Decimal::ZERO;
    let mut total_expenses = Decimal::ZERO;

    for tx in &in_range {
        let category = tx
            .category
            .clone()
            .unwrap_or_else(|| UNCATEGORIZED_LABEL.to_string());
        if tx.category.is_none() {
            uncategorized.push((*tx).clone());
        }

        if tx.amount.is_sign_positive() {
            *income_map.entry(category.clone()).or_insert(Decimal::ZERO) += tx.amount;
            total_income += tx.amount;
        } else {
            let magnitude = tx.amount.abs();
            *expense_map.entry(category.clone()).or_insert(Decimal::ZERO) += magnitude;
            total_expenses += magnitude;
        }

        if include_schedule_c {
            let sched_label = tx
                .schedule_c_line
                .clone()
                .unwrap_or_else(|| format!("Unmapped - {category}"));
            let signed = tx.amount;
            *schedule_c_map.entry(sched_label).or_insert(Decimal::ZERO) += signed;
        }
    }

    uncategorized.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));

    PnlReport {
        label: label.to_string(),
        from,
        to,
        income: to_sorted_lines(income_map),
        expenses: to_sorted_lines(expense_map),
        total_income,
        total_expenses,
        net: total_income - total_expenses,
        uncategorized,
        schedule_c: if include_schedule_c {
            Some(to_sorted_lines(schedule_c_map))
        } else {
            None
        },
    }
}

pub fn render_table(report: &PnlReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Profit & Loss — {} ({} to {})",
        report.label, report.from, report.to
    );
    out.push('\n');

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Category", "Amount"]);

    table.add_row(vec![
        Cell::new("INCOME").add_attribute(comfy_table::Attribute::Bold),
        Cell::new(""),
    ]);
    for line in &report.income {
        table.add_row(vec![line.category.clone(), format!("{:.2}", line.amount)]);
    }
    table.add_row(vec![
        "Total Income".to_string(),
        format!("{:.2}", report.total_income),
    ]);

    table.add_row(vec![
        Cell::new("EXPENSES").add_attribute(comfy_table::Attribute::Bold),
        Cell::new(""),
    ]);
    for line in &report.expenses {
        table.add_row(vec![line.category.clone(), format!("{:.2}", line.amount)]);
    }
    table.add_row(vec![
        "Total Expenses".to_string(),
        format!("{:.2}", report.total_expenses),
    ]);
    table.add_row(vec!["Net".to_string(), format!("{:.2}", report.net)]);

    let _ = writeln!(out, "{table}");

    if let Some(schedule_c) = &report.schedule_c {
        out.push('\n');
        let _ = writeln!(out, "Schedule C rollup:");
        let mut sched_table = Table::new();
        sched_table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Schedule C line", "Amount"]);
        for line in schedule_c {
            sched_table.add_row(vec![line.category.clone(), format!("{:.2}", line.amount)]);
        }
        let _ = writeln!(out, "{sched_table}");
    }

    if !report.uncategorized.is_empty() {
        out.push('\n');
        let _ = writeln!(
            out,
            "Uncategorized ({} transaction(s) — run `ledgerlite categorize`):",
            report.uncategorized.len()
        );
        let mut unc_table = Table::new();
        unc_table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Date", "Account", "Description", "Amount"]);
        for tx in &report.uncategorized {
            unc_table.add_row(vec![
                tx.date.to_string(),
                tx.account.clone(),
                tx.description.clone(),
                format!("{:.2}", tx.amount),
            ]);
        }
        let _ = writeln!(out, "{unc_table}");
    }

    out
}

pub fn render_csv(report: &PnlReport) -> String {
    let mut wtr = csv::Writer::from_writer(vec![]);
    let _ = wtr.write_record(["section", "category", "amount"]);
    for line in &report.income {
        let _ = wtr.write_record(["income", &line.category, &line.amount.to_string()]);
    }
    let _ = wtr.write_record(["income", "TOTAL", &report.total_income.to_string()]);
    for line in &report.expenses {
        let _ = wtr.write_record(["expense", &line.category, &line.amount.to_string()]);
    }
    let _ = wtr.write_record(["expense", "TOTAL", &report.total_expenses.to_string()]);
    let _ = wtr.write_record(["net", "NET", &report.net.to_string()]);
    if let Some(schedule_c) = &report.schedule_c {
        for line in schedule_c {
            let _ = wtr.write_record(["schedule_c", &line.category, &line.amount.to_string()]);
        }
    }
    for tx in &report.uncategorized {
        let _ = wtr.write_record([
            "uncategorized",
            &format!("{} | {} | {}", tx.date, tx.account, tx.description),
            &tx.amount.to_string(),
        ]);
    }
    String::from_utf8(wtr.into_inner().unwrap_or_default()).unwrap_or_default()
}

pub fn render_json(report: &PnlReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

pub fn render_markdown(report: &PnlReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Profit & Loss — {}", report.label);
    let _ = writeln!(out, "\n_{} to {}_\n", report.from, report.to);

    let _ = writeln!(out, "## Income\n");
    let _ = writeln!(out, "| Category | Amount |");
    let _ = writeln!(out, "|---|---:|");
    for line in &report.income {
        let _ = writeln!(out, "| {} | {:.2} |", line.category, line.amount);
    }
    let _ = writeln!(out, "| **Total Income** | **{:.2}** |", report.total_income);

    let _ = writeln!(out, "\n## Expenses\n");
    let _ = writeln!(out, "| Category | Amount |");
    let _ = writeln!(out, "|---|---:|");
    for line in &report.expenses {
        let _ = writeln!(out, "| {} | {:.2} |", line.category, line.amount);
    }
    let _ = writeln!(
        out,
        "| **Total Expenses** | **{:.2}** |",
        report.total_expenses
    );
    let _ = writeln!(out, "\n**Net: {:.2}**\n", report.net);

    if let Some(schedule_c) = &report.schedule_c {
        let _ = writeln!(out, "## Schedule C rollup\n");
        let _ = writeln!(out, "| Line | Amount |");
        let _ = writeln!(out, "|---|---:|");
        for line in schedule_c {
            let _ = writeln!(out, "| {} | {:.2} |", line.category, line.amount);
        }
        out.push('\n');
    }

    if !report.uncategorized.is_empty() {
        let _ = writeln!(
            out,
            "## Uncategorized ({} transaction(s))\n",
            report.uncategorized.len()
        );
        let _ = writeln!(out, "| Date | Account | Description | Amount |");
        let _ = writeln!(out, "|---|---|---|---:|");
        for tx in &report.uncategorized {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {:.2} |",
                tx.date, tx.account, tx.description, tx.amount
            );
        }
    }

    out
}

/// Parses `--format` values, kept separate from clap's own `ValueEnum` so
/// the error carries our friendly `UnknownFormat` message.
pub fn parse_format(raw: &str) -> Result<ReportFormat, String> {
    ReportFormat::from_str(raw).map_err(|_| raw.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Table,
    Csv,
    Json,
    Markdown,
}

impl FromStr for ReportFormat {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(ReportFormat::Table),
            "csv" => Ok(ReportFormat::Csv),
            "json" => Ok(ReportFormat::Json),
            "md" | "markdown" => Ok(ReportFormat::Markdown),
            _ => Err(()),
        }
    }
}

pub fn render(report: &PnlReport, format: ReportFormat) -> String {
    match format {
        ReportFormat::Table => render_table(report),
        ReportFormat::Csv => render_csv(report),
        ReportFormat::Json => render_json(report),
        ReportFormat::Markdown => render_markdown(report),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tx(date: &str, category: Option<&str>, amount: Decimal, sched: Option<&str>) -> Transaction {
        Transaction {
            id: format!("{date}-{amount}"),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            account: "Chase Checking".into(),
            description: "test".into(),
            raw_description: "test".into(),
            amount,
            category: category.map(|c| c.to_string()),
            schedule_c_line: sched.map(|s| s.to_string()),
            source_file: "f.csv".into(),
            occurrence: 0,
        }
    }

    #[test]
    fn totals_income_and_expenses_separately() {
        let txs = vec![
            tx("2026-09-01", Some("Sales Income"), dec!(1000.00), None),
            tx("2026-09-02", Some("Software"), dec!(-50.00), None),
            tx("2026-09-03", Some("Software"), dec!(-25.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        assert_eq!(report.total_income, dec!(1000.00));
        assert_eq!(report.total_expenses, dec!(75.00));
        assert_eq!(report.net, dec!(925.00));
        assert_eq!(report.expenses.len(), 1);
        assert_eq!(report.expenses[0].amount, dec!(75.00));
    }

    #[test]
    fn excludes_transactions_outside_range() {
        let txs = vec![
            tx("2026-08-31", Some("Software"), dec!(-10.00), None),
            tx("2026-09-01", Some("Software"), dec!(-20.00), None),
            tx("2026-10-01", Some("Software"), dec!(-30.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        assert_eq!(report.total_expenses, dec!(20.00));
    }

    #[test]
    fn tracks_uncategorized_separately() {
        let txs = vec![
            tx("2026-09-01", None, dec!(-10.00), None),
            tx("2026-09-02", Some("Software"), dec!(-20.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        assert_eq!(report.uncategorized.len(), 1);
        // Uncategorized transactions still count in expense totals.
        assert_eq!(report.total_expenses, dec!(30.00));
    }

    #[test]
    fn schedule_c_rollup_groups_by_line() {
        let txs = vec![
            tx(
                "2026-09-01",
                Some("Software"),
                dec!(-20.00),
                Some("Line 27a"),
            ),
            tx(
                "2026-09-02",
                Some("Software"),
                dec!(-5.00),
                Some("Line 27a"),
            ),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", true);
        let schedule_c = report.schedule_c.unwrap();
        assert_eq!(schedule_c.len(), 1);
        assert_eq!(schedule_c[0].amount, dec!(-25.00));
    }

    #[test]
    fn csv_and_json_and_markdown_render_without_panicking() {
        let txs = vec![tx("2026-09-01", Some("Software"), dec!(-20.00), None)];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        assert!(render_csv(&report).contains("expense"));
        assert!(render_json(&report).contains("total_expenses"));
        assert!(render_markdown(&report).contains("# Profit & Loss"));
        assert!(render_table(&report).contains("Profit & Loss"));
    }

    #[test]
    fn parse_format_accepts_known_values() {
        assert_eq!(parse_format("table").unwrap(), ReportFormat::Table);
        assert_eq!(parse_format("CSV").unwrap(), ReportFormat::Csv);
        assert_eq!(parse_format("json").unwrap(), ReportFormat::Json);
        assert_eq!(parse_format("md").unwrap(), ReportFormat::Markdown);
        assert!(parse_format("yaml").is_err());
    }
}
