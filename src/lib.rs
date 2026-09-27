//! ledgerlite — a fast, private, offline CLI that turns bank and
//! credit-card CSV exports into categorized books and a profit-and-loss
//! report.
//!
//! Data never leaves the machine: every command reads and writes local
//! files only.
//!
//! ledgerlite is a bookkeeping tool. It is **not tax advice** — consult a
//! qualified professional (CPA/EA) before making filing decisions.

pub mod config;
pub mod error;
pub mod money;
pub mod overrides;
pub mod profiles;
pub mod rules;
pub mod transaction;
