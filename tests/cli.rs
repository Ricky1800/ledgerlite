//! Integration tests that drive the compiled `ledgerlite` binary over the
//! fixture CSVs in `tests/fixtures/` (all fake data).

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn ledgerlite() -> Command {
    Command::cargo_bin("ledgerlite").expect("binary should build")
}

fn init_project() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    ledgerlite()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

#[test]
fn init_creates_config_and_rules_files() {
    let dir = init_project();
    assert!(dir.path().join("ledgerlite.toml").is_file());
    assert!(dir.path().join("rules.toml").is_file());
}

#[test]
fn init_refuses_to_clobber_existing_files_without_force() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_force_overwrites_existing_files() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args(["init", "--force"])
        .assert()
        .success();
}

#[test]
fn help_footer_mentions_not_tax_advice() {
    ledgerlite().arg("--help").assert().success().stdout(
        predicate::str::contains("not tax advice").or(predicate::str::contains("NOT tax advice")),
    );
}

#[test]
fn import_chase_checking_then_report_shows_totals() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 7 new transaction(s)"));

    ledgerlite()
        .current_dir(dir.path())
        .arg("categorize")
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--month", "2026-09", "--format", "table"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profit & Loss"))
        .stdout(predicate::str::contains("Software"))
        .stdout(predicate::str::contains("Total Income"));
}

#[test]
fn report_json_format_contains_expected_fields() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();
    ledgerlite()
        .current_dir(dir.path())
        .arg("categorize")
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args([
            "report",
            "--month",
            "2026-09",
            "--format",
            "json",
            "--schedule-c",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_income\""))
        .stdout(predicate::str::contains("\"schedule_c\""));
}

#[test]
fn report_csv_and_markdown_formats_work() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--year", "2026", "--format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("section,category,amount"));

    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--year", "2026", "--format", "md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Profit & Loss"));
}

#[test]
fn uncategorized_transactions_are_listed_before_categorize_runs() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--month", "2026-09"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Uncategorized"));
}

#[test]
fn categorize_dry_run_does_not_write_changes() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args(["categorize", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"));

    // Nothing should be categorized yet, so the report still shows an
    // Uncategorized section.
    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--month", "2026-09"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Uncategorized"));
}

#[test]
fn reimporting_overlapping_export_does_not_duplicate() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 7 new transaction(s)"));

    // chase_checking_overlap.csv repeats the first three rows and adds
    // exactly one genuinely new transaction (Costco).
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_overlap.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 1 new transaction(s)"))
        .stdout(predicate::str::contains("3 duplicate(s) skipped"));
}

#[test]
fn genuine_same_day_duplicates_are_both_kept() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("same_day_duplicates.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 new transaction(s)"));
}

#[test]
fn missing_column_produces_friendly_error() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("malformed_missing_amount_column.csv")
                .to_str()
                .unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Amount"))
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn import_without_init_reports_no_config() {
    let dir = tempfile::tempdir().unwrap();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ledgerlite init"));
}

#[test]
fn chase_credit_profile_imports_with_account_default() {
    let dir = init_project();
    // "Chase Credit Card" is registered by `init` with the chase-credit
    // profile as its default, so no --profile flag is needed here.
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_credit_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Credit Card",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 4 new transaction(s)"));
}

#[test]
fn bofa_profile_imports_with_explicit_flag() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("bofa_sample.csv").to_str().unwrap(),
            "--account",
            "Bank of America Checking",
            "--profile",
            "bofa",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 4 new transaction(s)"));
}

#[test]
fn capital_one_profile_combines_debit_credit_columns() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("capital_one_sample.csv").to_str().unwrap(),
            "--account",
            "Capital One",
            "--profile",
            "capital-one",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 3 new transaction(s)"));

    ledgerlite()
        .current_dir(dir.path())
        .args(["export", "--format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-32.99"))
        .stdout(predicate::str::contains("45.00"));
}

#[test]
fn amex_profile_negates_sign_convention() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("amex_sample.csv").to_str().unwrap(),
            "--account",
            "Amex Business",
            "--profile",
            "amex",
        ])
        .assert()
        .success();

    // Amex reports charges as positive numbers; ledgerlite must flip
    // them to negative (money out) on import.
    ledgerlite()
        .current_dir(dir.path())
        .args(["export", "--format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-54.99"))
        .stdout(predicate::str::contains("-412.30"));
}

#[test]
fn generic_profile_imports_with_default_column_mapping() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("generic_sample.csv").to_str().unwrap(),
            "--account",
            "Local Credit Union",
            "--profile",
            "generic",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 3 new transaction(s)"));
}

#[test]
fn report_html_format_writes_a_report_file() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();
    ledgerlite()
        .current_dir(dir.path())
        .arg("categorize")
        .assert()
        .success();

    let out_path = dir.path().join("report.html");
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "report",
            "--month",
            "2026-09",
            "--format",
            "html",
            "--schedule-c",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote report"));

    let html = std::fs::read_to_string(&out_path).unwrap();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>Profit &amp; Loss"));
    assert!(html.contains("summary-card"));
    assert!(html.contains("category-table"));
    assert!(html.contains("share-fill"));
    assert!(html.contains("Software"));
    assert!(html.contains("Schedule C rollup"));
    assert!(html.contains("not tax advice") || html.contains("NOT tax advice"));
}

#[test]
fn report_html_without_out_prints_to_stdout() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    ledgerlite()
        .current_dir(dir.path())
        .args(["report", "--month", "2026-09", "--format", "html"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<!DOCTYPE html>"));
}

#[test]
fn report_html_escapes_a_script_tag_in_a_transaction_description() {
    let dir = init_project();
    let csv_path = dir.path().join("evil.csv");
    std::fs::write(
        &csv_path,
        "Date,Description,Amount\n\
         2026-09-01,<script>alert(1)</script>,-25.00\n\
         2026-09-02,CLIENT INVOICE PAYMENT,500.00\n",
    )
    .unwrap();

    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            csv_path.to_str().unwrap(),
            "--account",
            "Chase Checking",
            "--profile",
            "generic",
        ])
        .assert()
        .success();

    let out_path = dir.path().join("report.html");
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "report",
            "--month",
            "2026-09",
            "--format",
            "html",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let html = std::fs::read_to_string(&out_path).unwrap();
    // The raw tag must never appear unescaped anywhere in the output...
    assert!(!html.contains("<script>alert(1)</script>"));
    // ...but its escaped form should, in the Uncategorized table.
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
}

#[test]
fn export_writes_to_output_file() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    let output_path = dir.path().join("export.json");
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "export",
            "--format",
            "json",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("\"amount\""));
}

#[test]
fn manual_override_survives_categorize() {
    let dir = init_project();
    ledgerlite()
        .current_dir(dir.path())
        .args([
            "import",
            fixture("chase_checking_sample.csv").to_str().unwrap(),
            "--account",
            "Chase Checking",
        ])
        .assert()
        .success();

    // Find the dedupe id for the Adobe transaction so we can override it.
    let ledger_path = dir.path().join("data").join("ledger.jsonl");
    let ledger_contents = std::fs::read_to_string(&ledger_path).unwrap();
    let adobe_line = ledger_contents
        .lines()
        .find(|l| l.contains("ADOBE"))
        .expect("adobe transaction present");
    let adobe_id = adobe_line
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

    let overrides_path = dir.path().join("overrides.toml");
    std::fs::write(
        &overrides_path,
        format!("[overrides]\n\"{adobe_id}\" = \"Owner Draw\"\n"),
    )
    .unwrap();

    ledgerlite()
        .current_dir(dir.path())
        .arg("categorize")
        .assert()
        .success();

    let updated = std::fs::read_to_string(&ledger_path).unwrap();
    let adobe_line = updated.lines().find(|l| l.contains("ADOBE")).unwrap();
    assert!(adobe_line.contains("\"Owner Draw\""));
    // Rules would have otherwise put this in Software — confirm it did not.
    assert!(!adobe_line.contains("\"Software\""));
}
