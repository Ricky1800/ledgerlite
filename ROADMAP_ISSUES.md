# Roadmap issues

These are scoped, ready-to-file issues for anyone (including future me)
who wants to contribute. Each one is small enough to land as a single
focused PR. Copy a section into a new GitHub issue as-is.

---

## 1. Add more bank/credit union import profiles

**Labels:** `good first issue`, `help wanted`

**Body:**

We ship `chase-checking`, `chase-credit`, `bofa`, `capital-one`, and
`amex`. Real users bank all over. Add one or more of: Wells Fargo, US
Bank, PNC, Citi, Discover, Ally, Navy Federal, or a regional credit
union.

**Acceptance criteria:**

- New entry added to `built_in_profiles()` in `src/profiles.rs` and to
  `BUILT_IN_PROFILE_NAMES`.
- A fake fixture CSV added under `tests/fixtures/` with 3+ representative
  rows (including at least one negative and one positive amount).
- An integration test in `tests/cli.rs` importing the fixture and
  asserting the resulting signs/amounts.
- Profile documented in the README's profile reference table.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all
  pass.

---

## 2. OFX/QFX import support

**Labels:** `help wanted`

**Body:**

Many banks also offer Open Financial Exchange (OFX/QFX) downloads, which
are more structured than CSV and don't need a column-mapping profile at
all. Add `ledgerlite import <file.ofx> --account "..."` support
alongside CSV.

**Acceptance criteria:**

- New `src/ofx.rs` module parsing the `<STMTTRN>` transaction blocks
  (a hand-rolled parser is fine — OFX/SGML is simple enough not to need
  a new dependency; if a crate is used it must stay MIT/Apache-2.0
  compatible and be justified in the PR).
- File extension (`.ofx`/`.qfx`) auto-detected so `--profile` is not
  required for OFX files.
- Feeds into the same dedupe/ledger pipeline as CSV import — no
  duplicate code path for storage.
- Fixture file + integration test under `tests/fixtures/`.
- README updated with an OFX example.

---

## 3. Receipt linking

**Labels:** `help wanted`

**Body:**

Let a transaction reference a receipt file (PDF/image) on disk, so
"what was this $340 Amazon charge for" has an answer.

**Acceptance criteria:**

- `Transaction` gains an optional `receipt_path: Option<PathBuf>` field
  (backward compatible: old ledger files without it still load).
- New command or flag to attach a receipt, e.g.
  `ledgerlite attach-receipt <transaction-id> <file>` — validates the
  file exists, does **not** copy/move it (this is offline bookkeeping,
  not a receipt vault).
- `ledgerlite export` includes the receipt path column when present.
- Unit tests for the new field's (de)serialization and the attach
  command's validation errors (missing transaction id, missing file).

---

## 4. `categorize --set` helper for manual overrides

**Labels:** `good first issue`

**Body:**

Today, `overrides.toml` is hand-edited. Add a small CLI ergonomics
command so users don't need to compute a transaction's dedupe key by
hand: `ledgerlite categorize --set <transaction-id> "<Category>"`.

**Acceptance criteria:**

- New `--set <id> <category>` flag/subcommand that writes to
  `overrides.toml` via the existing `Overrides` type and exits (does not
  also run the full categorize pass in the same invocation).
- Friendly error if `<transaction-id>` doesn't exist in the ledger.
- A way to discover ids: `ledgerlite report --format csv` or similar
  already exposes enough info to identify a transaction; document the
  workflow (find transaction -> get id -> `categorize --set`) in the
  README.
- Integration test covering set + a subsequent `categorize` run
  confirming the override sticks.

---

## 5. Budget vs. actual report

**Labels:** `help wanted`

**Body:**

Add an optional budget file (`budget.toml`, category -> monthly target)
and a `ledgerlite report --budget` mode that shows actual vs. budget
per category alongside the existing P&L.

**Acceptance criteria:**

- `budget.toml` schema documented in the README, loaded only when
  `--budget` is passed (no behavior change otherwise).
- Missing budget file with `--budget` passed produces a friendly error,
  not a panic.
- Table/CSV/JSON/MD renderers all handle the extra column(s).
- Unit tests for variance calculation (over/under budget, category with
  no budget entry, category with no actuals).

---

## 6. Shell completions

**Labels:** `good first issue`

**Body:**

Generate shell completions using `clap_complete` for bash/zsh/fish/
PowerShell.

**Acceptance criteria:**

- New `ledgerlite completions <shell>` subcommand (hidden from `--help`
  is fine) that prints the completion script to stdout.
- `clap_complete` added as a normal dependency (it's tiny and
  clap-maintained, consistent with "keep deps lean").
- README install section gets a one-line snippet per shell for wiring
  it up.
- Smoke-test in `tests/cli.rs` that each shell variant runs without
  erroring.

---

## 7. Multi-currency awareness

**Labels:** `help wanted`

**Body:**

Right now ledgerlite silently assumes every account is in the same
currency. At minimum, warn (or refuse) when accounts in
`ledgerlite.toml` don't agree, and stamp reports with the currency
symbol/code from `[general] currency`.

**Acceptance criteria:**

- `[general] currency` (ISO 4217 code, e.g. `"USD"`) added to
  `ledgerlite.toml`, defaulting to `USD` for backward compatibility.
- Reports show the currency code in the header.
- Explicitly **not** in scope for this issue: FX conversion between
  currencies — that's a much bigger feature and should be its own issue
  if ever pursued.
- Unit test for the default-currency backward-compat case (old config
  files without `[general] currency` still load).

---

## 8. Interactive review mode for uncategorized transactions

**Labels:** `help wanted`

**Body:**

`ledgerlite report` already lists uncategorized transactions. Add an
interactive terminal mode, `ledgerlite categorize --interactive`, that
walks through each uncategorized transaction one at a time and lets the
user pick/type a category, writing straight to `overrides.toml`.

**Acceptance criteria:**

- No new TUI framework dependency unless discussed first in the issue —
  a simple stdin prompt loop is enough for v1.
- Ctrl-C / EOF exits cleanly without corrupting `overrides.toml`
  (write after each answer, not just at the end).
- Skips transactions that already have a category or override.
- Integration test simulating stdin input via `assert_cmd`'s
  `write_stdin`.
