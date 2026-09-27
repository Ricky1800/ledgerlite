# ledgerlite

A fast, private, offline command-line tool that turns your bank and
credit-card CSV exports into categorized books and a profit-and-loss
report. Built for sole proprietors and small businesses who don't need
(or can't justify the cost of) QuickBooks, but still want real books.

**Data never leaves your machine.** ledgerlite reads CSV files you
already downloaded and writes plain-text files back to a folder you
control. No account, no cloud sync, no telemetry.

> **Not tax advice.** ledgerlite helps you organize transactions and see
> totals. It does not file taxes, does not know your specific tax
> situation, and the Schedule C line names it can attach to categories
> are a starting point for a conversation with a CPA/EA — not a
> substitute for one.

## Why

Most "free" bookkeeping tools want your bank credentials and a monthly
subscription. If you already export CSVs from your bank's website once a
month, ledgerlite gets you 90% of the way to a P&L in about five
minutes, offline, for free, and your data stays on your laptop.

## Install

```sh
cargo install --git https://github.com/Ricky1800/ledgerlite
```

This requires a Rust toolchain (1.75+). If you don't have one:
<https://rustup.rs>.

## 5-minute walkthrough

This walkthrough uses the fake fixture data shipped in the repo under
`tests/fixtures/` so you can follow along with real command output.

### 1. Initialize a project

```console
$ ledgerlite init
Created ledgerlite.toml
Created rules.toml

Next steps:
  1. Edit the [[account]] entries in ledgerlite.toml to match your real bank/card names.
  2. Run: ledgerlite import <file.csv> --account "Chase Checking"
  3. Run: ledgerlite categorize
  4. Run: ledgerlite report --month 2026-09
```

This writes two files in the current directory:

- **`ledgerlite.toml`** — your accounts (name + which import profile to
  use for each) and the built-in CSV column-mapping profiles.
- **`rules.toml`** — starter categorization rules for common small
  business categories.

Nothing is written outside the current directory; the ledger itself
lives under `./data/` and is created on first import.

### 2. Import a CSV export

```console
$ ledgerlite import tests/fixtures/chase_checking_sample.csv --account "Chase Checking"
Imported 7 new transaction(s) from 'tests/fixtures/chase_checking_sample.csv' into account 'Chase Checking' (7 row(s) read, 0 duplicate(s) skipped).
```

`--account` must match a name you defined in `ledgerlite.toml` (so
ledgerlite knows which import profile to use) — or you can pass
`--profile <name>` explicitly and skip registering the account at all:

```sh
ledgerlite import statement.csv --account "Bank of America Checking" --profile bofa
```

Re-running the same import (or an overlapping export covering some of
the same dates) is safe — already-seen transactions are silently
skipped:

```console
$ ledgerlite import tests/fixtures/chase_checking_overlap.csv --account "Chase Checking"
Imported 1 new transaction(s) from 'tests/fixtures/chase_checking_overlap.csv' into account 'Chase Checking' (4 row(s) read, 3 duplicate(s) skipped).
```

### 3. Categorize

```console
$ ledgerlite categorize
2026-09-02 Chase Checking | ADOBE  *CREATIVE CLOUD | -54.99: Uncategorized -> Software
2026-09-03 Chase Checking | SHELL OIL 57443210 | -41.10: Uncategorized -> Fuel
2026-09-03 Chase Checking | STARBUCKS STORE #04521 | -6.25: Uncategorized -> Meals
2026-09-05 Chase Checking | DEPOSIT CUSTOMER INVOICE 1042 | 1250.00: Uncategorized -> Sales Income
2026-09-07 Chase Checking | STAPLES STORE 0912 | -32.47: Uncategorized -> Supplies
2026-09-10 Chase Checking | MONTHLY SERVICE FEE | -12.00: Uncategorized -> Bank Fees
2026-09-12 Chase Checking | OWNER DRAW TRANSFER | -500.00: Uncategorized -> Owner Draw

7 change(s) applied.
```

Use `--dry-run` to preview changes without writing them. Rules are
applied in order, first match wins — see `rules.toml`. Anything you
categorize by hand goes in `overrides.toml`, keyed by the transaction's
stable id (shown in `ledgerlite export`), and `categorize` will never
overwrite an override.

### 4. Run the report

```console
$ ledgerlite report --month 2026-09
Profit & Loss — 2026-09 (2026-09-01 to 2026-09-30)

┌────────────────┬─────────┐
│ Category       ┆ Amount  │
╞════════════════╪═════════╡
│ INCOME         ┆         │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Sales Income   ┆ 1250.00 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Total Income   ┆ 1250.00 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ EXPENSES       ┆         │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Owner Draw     ┆ 500.00  │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Software       ┆ 54.99   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Fuel           ┆ 41.10   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Supplies       ┆ 32.47   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Bank Fees      ┆ 12.00   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Meals          ┆ 6.25    │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Total Expenses ┆ 646.81  │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┤
│ Net            ┆ 603.19  │
└────────────────┴─────────┘
```

Add `--schedule-c` for an IRS Schedule C line rollup, and
`--format csv|json|md` to pipe the report elsewhere. Any transaction
without a category shows up in its own "Uncategorized" section so
nothing silently falls through the cracks.

### 4b. Generate a printable HTML report

```console
$ ledgerlite report --year 2026 --format html --schedule-c --out report.html
Wrote report to 'report.html'.
```

Produces a single, self-contained, accountant-ready HTML file (inline
CSS, print stylesheet, no external assets or network calls) with:

- A summary header: total income, total expenses, net, and the period.
- Income/expense category tables, each row with a proportional **share
  bar** so the biggest categories are visible at a glance, not just
  readable as numbers.
- A **monthly trend chart** (inline SVG, no JS) when the report spans
  more than one month — income and expense bars per month.
- The same **Uncategorized** review section as the other formats.
- A **Schedule C rollup** table when `--schedule-c` is passed.

Every piece of transaction-derived text (description, account name) is
HTML-escaped before being written into the page, so a bank export
containing `&`, `<`, `>`, or quote characters — accidentally or
otherwise — can't break the page or inject markup.

Open it in any browser, or print it (`Ctrl/Cmd+P`) straight to PDF for a
client or accountant — the print stylesheet keeps tables from splitting
awkwardly across pages.

*What it looks like:* a serif, letter-styled page — four stat cards
(Income / Expenses / Net / Period) across the top, then an "Income"
table and an "Expenses" table where every row has a small horizontal
bar next to the amount sized to that category's share of the section
total, a compact green/red bar chart of monthly income vs. expenses
underneath, an optional Schedule C table, and a plain data table of any
still-uncategorized transactions at the bottom. No images are checked
into this repo (no build step, no assets, by design) — generate one
yourself with the command above to see it.

### 5. Export for your accountant

```console
$ ledgerlite export --format csv --output 2026-09-books.csv
Exported 7 transaction(s) to '2026-09-books.csv'.
```

## Commands

| Command | What it does |
|---|---|
| `ledgerlite init [--force]` | Creates `ledgerlite.toml` and `rules.toml` |
| `ledgerlite import <file> --account <name> [--profile <name>]` | Imports a CSV export |
| `ledgerlite categorize [--dry-run]` | Applies rules + manual overrides |
| `ledgerlite report [--month YYYY-MM \| --year YYYY \| --from YYYY-MM-DD --to YYYY-MM-DD] [--format table\|csv\|json\|md\|html] [--schedule-c] [--out <file>]` | P&L report (prints to stdout, or writes to `--out`) |
| `ledgerlite export [--format csv\|json] [--output <file>] [--from ...] [--to ...]` | Dumps the ledger |

Run `ledgerlite <command> --help` for full flag documentation.

## Import profile reference

An import profile tells ledgerlite which CSV columns to read. Built-in
profiles (all defined in the `[profiles.*]` tables of `ledgerlite.toml`,
so you can copy/edit them freely):

| Profile | Date column | Amount handling | Sign quirk |
|---|---|---|---|
| `chase-checking` | Posting Date (`%m/%d/%Y`) | single `Amount` column | none |
| `chase-credit` | Transaction Date (`%m/%d/%Y`) | single `Amount` column | none |
| `bofa` | Date (`%m/%d/%Y`) | single `Amount` column | none |
| `capital-one` | Transaction Date (`%Y-%m-%d`) | separate `Debit`/`Credit` columns | none |
| `amex` | Date (`%m/%d/%Y`) | single `Amount` column | **negated** — Amex reports charges as positive |
| `generic` | fully configurable | `amount_column` OR `debit_column`/`credit_column` | configurable |

ledgerlite's internal convention is **negative = money out (expenses),
positive = money in (income)**. Most banks already export this way;
Amex is the notable exception, which is why its profile sets
`sign = "negated"`.

To add your own bank, edit (or copy) the `generic` table:

```toml
[profiles.my-credit-union]
date_column = "Date"
date_format = "%Y-%m-%d"
description_column = "Description"
amount_column = "Amount"      # or use debit_column/credit_column instead
sign = "as_is"                 # or "negated" if charges show as positive
```

Then either add an `[[account]]` entry pointing at it, or pass
`--profile my-credit-union` on import.

### A note on deduping

The dedupe key is `date + amount + normalized description + account +
occurrence index`. The occurrence index is what lets two genuinely
identical same-day purchases (two separate $4.50 coffees) both stay in
your books, while re-importing an overlapping export of the same file
is recognized as duplicates and skipped. This is computed per import
file (see `src/dedupe.rs` for the exact algorithm and its one known
edge case around genuinely-new same-day duplicates arriving in a later,
non-overlapping import).

## Rules reference

`rules.toml` holds an ordered list of `[[rule]]` tables. The first
matching rule wins:

```toml
[[rule]]
name = "Software subscriptions"
match_type = "regex"                 # contains | regex | amount_range | account
pattern = "(?i)adobe|figma|notion"
category = "Software"
schedule_c_line = "Line 27a - Other expenses (Software)"
```

- `contains` — case-insensitive substring match against the description.
- `regex` — regex match against the description.
- `amount_range` — matches when the transaction's absolute amount falls
  within `min_amount`/`max_amount` (either may be omitted).
- `account` — case-insensitive substring match against the account name.

## Manual overrides

`overrides.toml` maps a transaction id (shown by `ledgerlite export`) to
a category you chose by hand:

```toml
[overrides]
"2026-09-02|Chase Checking|-54.99|adobe *creative cloud|0" = "Software - Annual Plan"
```

`ledgerlite categorize` always applies overrides first and never
replaces what it finds there, no matter what the rules would otherwise
say.

## Privacy

ledgerlite makes network requests for exactly one thing: `cargo install`
fetching the source/dependencies once. After that, every command reads
and writes only local files in your project directory. There is no
telemetry, no analytics, and no server component. Back up `data/`,
`ledgerlite.toml`, `rules.toml`, and `overrides.toml` however you already
back up the rest of your files (they're all plain text).

## Roadmap

See [`ROADMAP_ISSUES.md`](ROADMAP_ISSUES.md) for scoped, ready-to-pick-up
future work: more bank profiles, OFX/QFX import, receipt linking, a
`categorize --set` override helper, budget vs. actual reporting, shell
completions, multi-currency awareness, and an interactive categorization
mode.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Authors

- [@Ricky1800](https://github.com/Ricky1800)
- [@orbitwebsites-cloud](https://github.com/orbitwebsites-cloud) ([OrbitBoyzz](https://orbitboyzz.me))

## License

MIT — see [`LICENSE`](LICENSE).
