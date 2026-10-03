//! Command-line argument definitions (clap derive).

use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub const NOT_TAX_ADVICE: &str = "ledgerlite is a bookkeeping tool. It is NOT tax advice. \
Consult a qualified professional (CPA/EA) before making tax filing decisions.";

#[derive(Parser)]
#[command(
    name = "ledgerlite",
    version,
    about = "Turn bank/credit-card CSV exports into categorized books and a profit-and-loss report — entirely offline.",
    after_help = NOT_TAX_ADVICE
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create ledgerlite.toml and rules.toml with sensible starter content
    Init {
        /// Overwrite ledgerlite.toml / rules.toml if they already exist
        #[arg(long)]
        force: bool,
    },

    /// Import a bank/credit-card CSV export into the local ledger
    Import {
        /// Path to the CSV file to import
        file: PathBuf,

        /// Account name this file belongs to, e.g. "Chase Checking"
        #[arg(long)]
        account: String,

        /// Import profile to use (defaults to the account's configured
        /// profile in ledgerlite.toml); built-ins: chase-checking,
        /// chase-credit, bofa, capital-one, amex, generic
        #[arg(long)]
        profile: Option<String>,
    },

    /// Apply categorization rules (and manual overrides) to the ledger
    Categorize {
        /// Show what would change without writing anything
        #[arg(long)]
        dry_run: bool,

        /// Manually assign a category override to a specific transaction: --set <id> <category>
        #[arg(long, num_args = 2, value_names = ["ID", "CATEGORY"])]
        set: Option<Vec<String>>,
    },

    /// Generate a profit & loss report
    Report {
        /// Report a single calendar month, e.g. 2026-09
        #[arg(long)]
        month: Option<String>,

        /// Report a full calendar year, e.g. 2026
        #[arg(long)]
        year: Option<i32>,

        /// Start of a custom date range (YYYY-MM-DD, inclusive)
        #[arg(long)]
        from: Option<String>,

        /// End of a custom date range (YYYY-MM-DD, inclusive)
        #[arg(long)]
        to: Option<String>,

        /// Output format: table, csv, json, md, or html
        #[arg(long, default_value = "table")]
        format: String,

        /// Also include an IRS Schedule C line rollup
        #[arg(long)]
        schedule_c: bool,

        /// Write the report to this file instead of stdout (e.g. --format
        /// html --out report.html)
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Export transactions to CSV/JSON for an accountant
    Export {
        /// Output format: csv or json
        #[arg(long, default_value = "csv")]
        format: String,

        /// Write to this file instead of stdout
        #[arg(long)]
        output: Option<PathBuf>,

        /// Start of a custom date range (YYYY-MM-DD, inclusive)
        #[arg(long)]
        from: Option<String>,

        /// End of a custom date range (YYYY-MM-DD, inclusive)
        #[arg(long)]
        to: Option<String>,
    },
}
