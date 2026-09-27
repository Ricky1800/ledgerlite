//! Error types for ledgerlite.
//!
//! Every variant is written to answer three questions a user actually has
//! when something goes wrong: *which file*, *which line/column*, and *what
//! exactly* was wrong with it. Plain `anyhow::anyhow!("bad input")` style
//! errors are avoided anywhere the user would need to go hunting.

use std::path::PathBuf;
use thiserror::Error;

/// Convenience result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, LedgerError>;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("could not read '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write '{path}': {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not create directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse TOML in '{path}': {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("failed to serialize TOML for '{path}': {source}")]
    TomlSerialize {
        path: PathBuf,
        #[source]
        source: Box<toml::ser::Error>,
    },

    #[error("{path} already exists (use --force to overwrite)")]
    AlreadyExists { path: PathBuf },

    #[error("CSV error reading '{path}': {source}")]
    Csv {
        path: PathBuf,
        #[source]
        source: Box<csv::Error>,
    },

    #[error("'{path}' has no header row, so column names cannot be matched")]
    MissingHeader { path: PathBuf },

    #[error(
        "column '{column}' not found in the header of '{path}' (available columns: {available})"
    )]
    MissingColumn {
        path: PathBuf,
        column: String,
        available: String,
    },

    #[error(
        "row {row} of '{path}': could not parse date '{value}' in column '{column}' using format '{format}'"
    )]
    DateParse {
        path: PathBuf,
        row: usize,
        column: String,
        value: String,
        format: String,
    },

    #[error(
        "row {row} of '{path}': could not parse amount '{value}' in column '{column}': {reason}"
    )]
    AmountParse {
        path: PathBuf,
        row: usize,
        column: String,
        value: String,
        reason: String,
    },

    #[error(
        "row {row} of '{path}' has {found} columns but the header defines {expected} — the file may be malformed"
    )]
    RowShapeMismatch {
        path: PathBuf,
        row: usize,
        found: usize,
        expected: usize,
    },

    #[error(
        "import profile '{0}' is missing both an amount column and a debit/credit column pair"
    )]
    ProfileMissingAmount(String),

    #[error("unknown import profile '{0}' — check the [profiles.*] tables in ledgerlite.toml")]
    UnknownProfile(String),

    #[error(
        "account '{0}' is not defined in ledgerlite.toml and no --profile was given — add the account or pass --profile <name>"
    )]
    NoProfileForAccount(String),

    #[error("rule #{index} ('{name}') is invalid: {reason}")]
    InvalidRule {
        index: usize,
        name: String,
        reason: String,
    },

    #[error("rule #{index} ('{name}') has an invalid regex pattern: {source}")]
    InvalidRegex {
        index: usize,
        name: String,
        #[source]
        source: regex::Error,
    },

    #[error("json error while reading/writing '{path}': {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("no ledgerlite.toml found in the current directory — run `ledgerlite init` first")]
    NoConfig,

    #[error("invalid date range: {0}")]
    InvalidDateRange(String),

    #[error("invalid --month value '{0}' — expected YYYY-MM, e.g. 2026-09")]
    InvalidMonth(String),

    #[error("invalid --year value '{0}' — expected a four digit year, e.g. 2026")]
    InvalidYear(String),

    #[error("invalid date '{0}' — expected YYYY-MM-DD")]
    InvalidDate(String),

    #[error("--from/--to, --month, and --year are mutually exclusive — pass only one")]
    ConflictingDateRange,

    #[error("no date range given — pass --month, --year, or --from/--to")]
    MissingDateRange,

    #[error("unknown output format '{0}' (expected table, csv, json, or md)")]
    UnknownFormat(String),

    #[error("ledger data file '{path}' contains invalid JSON on line {line}: {source}")]
    LedgerLineParse {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
}
