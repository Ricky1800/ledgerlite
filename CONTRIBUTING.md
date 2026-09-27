# Contributing to ledgerlite

Thanks for considering a contribution. ledgerlite is a small, focused tool —
please keep changes in that spirit.

## Getting set up

```sh
git clone https://github.com/Ricky1800/ledgerlite.git
cd ledgerlite
cargo build
cargo test
```

You'll need a recent stable Rust toolchain (1.75+). No other tools or
network access are required to build or test.

## Before opening a PR

Run the same checks CI runs:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must be clean. `cargo fmt` (no `--check`) will fix formatting
for you.

## Code style

- Money is **always** `rust_decimal::Decimal`. Never introduce `f64`/`f32`
  for anything that represents a dollar amount.
- Errors should tell the user which file, which row, and which column
  went wrong wherever that information is available — see `src/error.rs`
  for the existing patterns. Avoid bare `anyhow!("something failed")`
  inside library code; that's fine in a script but not here.
- Prefer small, independently testable modules over one large file.
- New import profiles, rules, or report formats should come with unit
  tests, and anything user-facing (a new command or flag) should come
  with an integration test under `tests/`.

## Adding a bank/CSV import profile

Built-in profiles live in `src/profiles.rs`. To add one:

1. Add an entry to `built_in_profiles()` with the column mapping.
2. Add it to `BUILT_IN_PROFILE_NAMES`.
3. Add a fixture CSV under `tests/fixtures/` with a couple of
   representative (fake) rows.
4. Add an integration test in `tests/cli.rs` that imports the fixture and
   asserts the resulting amounts/signs are correct.
5. Mention the new profile in the README's profile reference table.

Real bank CSV formats occasionally change without notice — if you're
fixing a profile that broke rather than adding a new one, please
describe what changed in the PR description.

## Reporting bugs / requesting features

Use the issue templates under `.github/ISSUE_TEMPLATE/`. For bugs,
a small (fake/anonymized) CSV snippet that reproduces the problem is the
single most useful thing you can include.

## Scope

ledgerlite intentionally does **not** aim to be a full accounting system.
Before proposing a large feature (multi-currency, double-entry
bookkeeping, invoicing, etc.), please open an issue to discuss it first —
see `ROADMAP_ISSUES.md` for the kind of scope that's a good fit.
