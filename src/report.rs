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
    /// One entry per calendar month touched by the report's date range,
    /// oldest first — the data behind the HTML report's monthly trend
    /// chart (and generally useful in JSON export for anyone building
    /// their own chart from it).
    pub monthly: Vec<MonthlyTotal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyTotal {
    /// "YYYY-MM".
    pub month: String,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
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
    // (income, expenses) per "YYYY-MM"; BTreeMap keeps months in
    // chronological order since that string sorts lexicographically the
    // same as it sorts by date.
    let mut monthly_map: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
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

        let month_entry = monthly_map
            .entry(tx.date.format("%Y-%m").to_string())
            .or_insert((Decimal::ZERO, Decimal::ZERO));

        if tx.amount.is_sign_positive() {
            *income_map.entry(category.clone()).or_insert(Decimal::ZERO) += tx.amount;
            total_income += tx.amount;
            month_entry.0 += tx.amount;
        } else {
            let magnitude = tx.amount.abs();
            *expense_map.entry(category.clone()).or_insert(Decimal::ZERO) += magnitude;
            total_expenses += magnitude;
            month_entry.1 += magnitude;
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

    let monthly = monthly_map
        .into_iter()
        .map(|(month, (income, expenses))| MonthlyTotal {
            month,
            income,
            expenses,
            net: income - expenses,
        })
        .collect();

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
        monthly,
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
    Html,
}

impl FromStr for ReportFormat {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(ReportFormat::Table),
            "csv" => Ok(ReportFormat::Csv),
            "json" => Ok(ReportFormat::Json),
            "md" | "markdown" => Ok(ReportFormat::Markdown),
            "html" => Ok(ReportFormat::Html),
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
        ReportFormat::Html => render_html(report),
    }
}

/// Escapes text for safe inclusion in HTML: every transaction-derived
/// string (description, account, category, error text) goes through this
/// before being written into the report, so a bank export containing
/// `<`, `>`, `&`, `"`, or `'` — accidentally or, in the case of a
/// malicious CSV, deliberately — can never break out of its containing
/// tag or attribute.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Renders `pct` (0.0..=100.0, already clamped by the caller) as an inline
/// `width:` style percentage with two decimal places, for a share bar.
fn bar_width(amount: Decimal, max: Decimal) -> String {
    if max.is_zero() {
        return "0.00".to_string();
    }
    let pct = (amount.abs() / max.abs() * Decimal::from(100)).min(Decimal::from(100));
    format!("{:.2}", pct)
}

const HTML_STYLE: &str = r#"
  :root {
    --ink: #1a1a2e;
    --muted: #5b6472;
    --border: #dfe3e8;
    --bg: #ffffff;
    --bg-inset: #f6f8fa;
    --income: #15803d;
    --income-bg: #dcfce7;
    --expense: #b91c1c;
    --expense-bg: #fee2e2;
    --accent: #1e3a5f;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    padding: 2.5rem 2rem 4rem;
    background: var(--bg);
    color: var(--ink);
    font-family: Georgia, "Times New Roman", Times, serif;
    line-height: 1.5;
    font-size: 14px;
  }
  .wrap { max-width: 880px; margin: 0 auto; }
  h1 { font-size: 1.6rem; margin: 0 0 0.15rem; }
  .subtitle { color: var(--muted); margin: 0 0 1.75rem; font-size: 0.95rem; }
  .disclaimer {
    font-size: 0.75rem;
    color: var(--muted);
    border-left: 3px solid var(--border);
    padding-left: 0.65rem;
    margin: 0 0 2rem;
  }

  .summary-row {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin-bottom: 2.25rem;
  }
  .summary-card {
    flex: 1 1 160px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.85rem 1rem;
    background: var(--bg-inset);
  }
  .summary-card .label {
    display: block;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin-bottom: 0.25rem;
  }
  .summary-card .value { font-size: 1.25rem; font-weight: 700; }
  .summary-card.income .value { color: var(--income); }
  .summary-card.expense .value { color: var(--expense); }
  .summary-card.net .value.negative { color: var(--expense); }
  .summary-card.net .value.positive { color: var(--income); }

  h2 {
    font-size: 1.05rem;
    margin: 2.25rem 0 0.75rem;
    padding-bottom: 0.35rem;
    border-bottom: 2px solid var(--border);
  }

  table.category-table { width: 100%; border-collapse: collapse; }
  table.category-table td, table.category-table th {
    padding: 0.4rem 0.5rem;
    text-align: left;
    font-size: 0.85rem;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }
  table.category-table th { color: var(--muted); font-size: 0.7rem; text-transform: uppercase; }
  table.category-table td.amount { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  table.category-table td.share { width: 40%; }
  .share-track {
    background: var(--bg-inset);
    border-radius: 4px;
    height: 10px;
    overflow: hidden;
  }
  .share-fill { height: 100%; border-radius: 4px; }
  .share-fill.income { background: var(--income); }
  .share-fill.expense { background: var(--expense); }
  tr.total td { font-weight: 700; border-top: 2px solid var(--border); border-bottom: none; }

  .trend-chart { display: block; margin: 0.5rem 0 0; }
  .trend-chart text { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; }

  .uncategorized-note {
    font-size: 0.85rem;
    color: var(--muted);
    margin-bottom: 0.75rem;
  }
  table.tx-table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  table.tx-table th, table.tx-table td {
    padding: 0.35rem 0.5rem;
    border-bottom: 1px solid var(--border);
    text-align: left;
  }
  table.tx-table td.amount { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  table.tx-table th { color: var(--muted); font-size: 0.68rem; text-transform: uppercase; }

  footer {
    margin-top: 3rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border);
    font-size: 0.75rem;
    color: var(--muted);
  }

  @media print {
    body { padding: 0; font-size: 12px; }
    .summary-card, table.category-table, table.tx-table, .trend-chart { break-inside: avoid; }
    h2 { break-after: avoid; }
  }
"#;

fn render_category_table(
    title: &str,
    lines: &[CategoryLine],
    total: Decimal,
    css_class: &str,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, r#"<h2>{}</h2>"#, escape_html(title));
    let _ = writeln!(
        out,
        r#"<table class="category-table"><thead><tr><th>Category</th><th>Amount</th><th>Share</th></tr></thead><tbody>"#
    );
    let max = lines
        .iter()
        .map(|l| l.amount.abs())
        .max()
        .unwrap_or(Decimal::ZERO);
    for line in lines {
        let _ = writeln!(
            out,
            r#"<tr><td>{}</td><td class="amount">{:.2}</td><td class="share"><div class="share-track"><div class="share-fill {}" style="width:{}%"></div></div></td></tr>"#,
            escape_html(&line.category),
            line.amount,
            css_class,
            bar_width(line.amount, max),
        );
    }
    let _ = writeln!(
        out,
        r#"<tr class="total"><td>Total {}</td><td class="amount">{:.2}</td><td></td></tr>"#,
        escape_html(title),
        total,
    );
    out.push_str("</tbody></table>");
    out
}

/// Renders `report.monthly` as a small inline SVG bar chart: one pair of
/// bars per month (income above the axis, expenses below it), scaled to
/// the largest single value so every bar is comparable at a glance.
fn render_trend_chart(monthly: &[MonthlyTotal]) -> String {
    if monthly.is_empty() {
        return String::new();
    }
    let width = 640.0_f64;
    let height = 200.0_f64;
    let axis_y = height / 2.0;
    let half = axis_y - 20.0; // leave room for month labels top/bottom
    let n = monthly.len();
    let slot = width / n as f64;
    let bar_w = (slot * 0.32).max(2.0);

    let max = monthly
        .iter()
        .map(|m| m.income.max(m.expenses))
        .fold(Decimal::ZERO, |a, b| if b > a { b } else { a });
    let max_f: f64 = max.to_string().parse().unwrap_or(1.0);
    let max_f = if max_f <= 0.0 { 1.0 } else { max_f };

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg class="trend-chart" viewBox="0 0 {width} {height}" width="{width}" height="{height}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Monthly income and expense trend">"#,
    );
    let _ = write!(
        svg,
        r##"<line x1="0" y1="{axis_y}" x2="{width}" y2="{axis_y}" stroke="#dfe3e8" stroke-width="1" />"##,
    );

    for (i, m) in monthly.iter().enumerate() {
        let income_f: f64 = m.income.to_string().parse().unwrap_or(0.0);
        let expense_f: f64 = m.expenses.to_string().parse().unwrap_or(0.0);
        let cx = slot * i as f64 + slot / 2.0;
        let x = cx - bar_w / 2.0;

        let income_h = (income_f / max_f * half).max(0.0);
        let expense_h = (expense_f / max_f * half).max(0.0);

        let _ = write!(
            svg,
            r##"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.1}" height="{h:.1}" fill="#15803d"><title>{label} income: {income:.2}</title></rect>"##,
            x = x,
            y = axis_y - income_h,
            h = income_h,
            label = escape_html(&m.month),
            income = m.income,
        );
        let _ = write!(
            svg,
            r##"<rect x="{x:.1}" y="{axis_y:.1}" width="{bar_w:.1}" height="{h:.1}" fill="#b91c1c"><title>{label} expenses: {expenses:.2}</title></rect>"##,
            x = x,
            h = expense_h,
            label = escape_html(&m.month),
            expenses = m.expenses,
        );
        let _ = write!(
            svg,
            r##"<text x="{cx:.1}" y="{y:.1}" font-size="9" fill="#5b6472" text-anchor="middle">{label}</text>"##,
            cx = cx,
            y = height - 4.0,
            label = escape_html(&m.month),
        );
    }
    svg.push_str("</svg>");
    svg
}

pub fn render_html(report: &PnlReport) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Profit &amp; Loss — {label}</title>
<style>{style}</style>
</head>
<body>
<div class="wrap">
<h1>Profit &amp; Loss</h1>
<p class="subtitle">{label} &middot; {from} to {to}</p>
<p class="disclaimer">Generated by ledgerlite. {disclaimer}</p>
"#,
        label = escape_html(&report.label),
        style = HTML_STYLE,
        from = report.from,
        to = report.to,
        disclaimer = escape_html(crate::cli::NOT_TAX_ADVICE),
    );

    let net_class = if report.net.is_sign_negative() {
        "negative"
    } else {
        "positive"
    };
    let _ = write!(
        out,
        r#"<div class="summary-row">
<div class="summary-card income"><span class="label">Income</span><span class="value">{:.2}</span></div>
<div class="summary-card expense"><span class="label">Expenses</span><span class="value">{:.2}</span></div>
<div class="summary-card net"><span class="label">Net</span><span class="value {}">{:.2}</span></div>
<div class="summary-card"><span class="label">Period</span><span class="value">{} &ndash; {}</span></div>
</div>
"#,
        report.total_income, report.total_expenses, net_class, report.net, report.from, report.to,
    );

    out.push_str(&render_category_table(
        "Income",
        &report.income,
        report.total_income,
        "income",
    ));
    out.push_str(&render_category_table(
        "Expenses",
        &report.expenses,
        report.total_expenses,
        "expense",
    ));

    if report.monthly.len() > 1 {
        out.push_str("<h2>Monthly trend</h2>");
        out.push_str(&render_trend_chart(&report.monthly));
    }

    if let Some(schedule_c) = &report.schedule_c {
        let total: Decimal = schedule_c.iter().map(|l| l.amount).sum();
        out.push_str(&render_category_table(
            "Schedule C rollup",
            schedule_c,
            total,
            "expense",
        ));
    }

    let _ = write!(out, "<h2>Uncategorized</h2>");
    if report.uncategorized.is_empty() {
        out.push_str(r#"<p class="uncategorized-note">Every transaction in this period is categorized. Nothing to review.</p>"#);
    } else {
        let _ = write!(
            out,
            r#"<p class="uncategorized-note">{} transaction(s) don't have a category yet (grouped under &ldquo;Uncategorized&rdquo; above) &mdash; run <code>ledgerlite categorize</code> to review them:</p>"#,
            report.uncategorized.len(),
        );
        out.push_str(r#"<table class="tx-table"><thead><tr><th>Date</th><th>Account</th><th>Description</th><th>Amount</th></tr></thead><tbody>"#);
        for tx in &report.uncategorized {
            let _ = write!(
                out,
                r#"<tr><td>{}</td><td>{}</td><td>{}</td><td class="amount">{:.2}</td></tr>"#,
                tx.date,
                escape_html(&tx.account),
                escape_html(&tx.description),
                tx.amount,
            );
        }
        out.push_str("</tbody></table>");
    }

    let _ = write!(
        out,
        r#"<footer>ledgerlite &middot; offline bookkeeping &middot; not tax advice</footer>
</div>
</body>
</html>
"#
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tx(date: &str, category: Option<&str>, amount: Decimal, sched: Option<&str>) -> Transaction {
        Transaction {
            id: date.to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            account: "Chase Checking".into(),
            description: "test".into(),
            raw_description: "test".into(),
            amount,
            category: category.map(|s| s.to_string()),
            schedule_c_line: sched.map(|s| s.to_string()),
            source_file: "f.csv".into(),
            occurrence: 0,
            receipt_path: None,
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
        assert_eq!(parse_format("HTML").unwrap(), ReportFormat::Html);
        assert!(parse_format("yaml").is_err());
    }

    #[test]
    fn escape_html_neutralizes_every_special_character() {
        assert_eq!(
            escape_html("<script>alert('x')</script> & \"quoted\""),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;quoted&quot;"
        );
    }

    #[test]
    fn escape_html_leaves_plain_text_untouched() {
        assert_eq!(escape_html("Adobe Creative Cloud"), "Adobe Creative Cloud");
    }

    #[test]
    fn render_html_escapes_a_malicious_transaction_description() {
        let txs = vec![tx("2026-09-01", None, dec!(-25.00), None)];
        let mut txs = txs;
        txs[0].description = "<script>alert(1)</script>".to_string();

        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        let html = render_html(&report);

        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn render_html_includes_summary_and_category_tables() {
        let txs = vec![
            tx("2026-09-01", Some("Sales Income"), dec!(1000.00), None),
            tx("2026-09-02", Some("Software"), dec!(-50.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        let html = render_html(&report);

        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("summary-card"));
        assert!(html.contains("Sales Income"));
        assert!(html.contains("Software"));
        assert!(html.contains("1000.00"));
        assert!(html.contains("share-fill"));
        // No external assets: everything is inline.
        assert!(!html.contains("<link "));
        assert!(!html.contains("<script src"));
    }

    #[test]
    fn render_html_draws_a_trend_chart_across_multiple_months() {
        let txs = vec![
            tx("2026-07-15", Some("Sales Income"), dec!(500.00), None),
            tx("2026-08-15", Some("Sales Income"), dec!(700.00), None),
            tx("2026-09-15", Some("Sales Income"), dec!(900.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "Q3 2026", false);
        assert_eq!(report.monthly.len(), 3);
        assert_eq!(report.monthly[0].month, "2026-07");
        assert_eq!(report.monthly[2].month, "2026-09");

        let html = render_html(&report);
        assert!(html.contains("Monthly trend"));
        assert!(html.contains(r#"<svg class="trend-chart""#));
    }

    #[test]
    fn render_html_omits_trend_chart_for_a_single_month() {
        let txs = vec![tx("2026-09-01", Some("Sales Income"), dec!(500.00), None)];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        assert_eq!(report.monthly.len(), 1);
        let html = render_html(&report);
        assert!(!html.contains("Monthly trend"));
        assert!(!html.contains(r#"<svg class="trend-chart""#));
    }

    #[test]
    fn render_html_shows_no_uncategorized_note_when_fully_categorized() {
        let txs = vec![tx("2026-09-01", Some("Software"), dec!(-10.00), None)];
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "2026-09", false);
        let html = render_html(&report);
        assert!(html.contains("Nothing to review"));
    }

    #[test]
    fn monthly_totals_are_grouped_and_sorted_chronologically() {
        let txs = vec![
            tx("2026-09-01", Some("Sales Income"), dec!(100.00), None),
            tx("2026-08-01", Some("Sales Income"), dec!(200.00), None),
            tx("2026-08-15", Some("Software"), dec!(-30.00), None),
        ];
        let from = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let report = build_report(&txs, from, to, "range", false);
        assert_eq!(report.monthly.len(), 2);
        assert_eq!(report.monthly[0].month, "2026-08");
        assert_eq!(report.monthly[0].income, dec!(200.00));
        assert_eq!(report.monthly[0].expenses, dec!(30.00));
        assert_eq!(report.monthly[0].net, dec!(170.00));
        assert_eq!(report.monthly[1].month, "2026-09");
        assert_eq!(report.monthly[1].income, dec!(100.00));
    }

    #[test]
    fn bar_width_handles_zero_max_without_dividing_by_zero() {
        assert_eq!(bar_width(dec!(50.00), Decimal::ZERO), "0.00");
    }

    #[test]
    fn bar_width_caps_at_one_hundred_percent() {
        assert_eq!(bar_width(dec!(50.00), dec!(50.00)), "100.00");
    }
}
