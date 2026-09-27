# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-26

### Added

- `ledgerlite init` — scaffolds `ledgerlite.toml` (accounts + import
  profiles) and `rules.toml` (starter categorization rules covering
  software, advertising, fuel, meals, supplies, bank fees, owner draw,
  and sales income, each optionally mapped to an IRS Schedule C line).
- `ledgerlite import` — imports a bank/credit-card CSV export into a
  local JSON Lines ledger, with built-in profiles for Chase checking,
  Chase credit card, Bank of America, Capital One, and American Express,
  plus a fully TOML-configurable `generic` profile for anything else.
- Stable dedupe key (date + amount + normalized description + account +
  occurrence index) so re-importing an overlapping export never creates
  duplicate transactions, while genuine same-day duplicate purchases are
  both kept.
- `ledgerlite categorize` — first-match-wins rule engine supporting
  `contains`, `regex`, `amount_range`, and `account` matchers, a
  `--dry-run` mode, and a manual `overrides.toml` file that rules never
  overwrite.
- `ledgerlite report` — profit & loss report for a month, year, or custom
  date range, in `table`, `csv`, `json`, or `md` format, with an optional
  Schedule C line rollup and a dedicated "Uncategorized" section.
- `ledgerlite export` — CSV/JSON export of the ledger (optionally date
  filtered) for handing off to an accountant.
- All money handled via `rust_decimal::Decimal` — never `f64`.
- Friendly, specific errors: which file, which row, which column, and
  why, for every parsing failure.
- Unit tests for money parsing, dedupe, every import profile's sign
  convention, rule matching order, and report totals; integration tests
  driving the compiled binary over fixture CSVs.
- CI (GitHub Actions) running `cargo fmt --check`, `cargo clippy --
  -D warnings`, and `cargo test` on Ubuntu, Windows, and macOS.

[0.1.0]: https://github.com/Ricky1800/ledgerlite/releases/tag/v0.1.0
