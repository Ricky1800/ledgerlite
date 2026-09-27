//! Built-in CSV import profiles.
//!
//! These mirror the column layouts of real export formats as of 2026.
//! Banks change their export formats occasionally without notice; if a
//! built-in profile stops matching, edit the corresponding
//! `[profiles.*]` table in `ledgerlite.toml` — nothing here is hardcoded
//! into the binary at runtime, it is only used to seed the file that
//! `ledgerlite init` writes.

use crate::config::{ImportProfile, SignConvention};
use std::collections::HashMap;

/// Names of every profile shipped out of the box, in the order they
/// should appear in a freshly generated `ledgerlite.toml`.
pub const BUILT_IN_PROFILE_NAMES: [&str; 6] = [
    "chase-checking",
    "chase-credit",
    "bofa",
    "capital-one",
    "amex",
    "generic",
];

/// Returns the full set of built-in import profiles.
pub fn built_in_profiles() -> HashMap<String, ImportProfile> {
    let mut map = HashMap::new();

    // Chase checking/savings export: Details,Posting Date,Description,
    // Amount,Type,Balance,Check or Slip #
    // Amount is already signed: debits negative, credits positive.
    map.insert(
        "chase-checking".to_string(),
        ImportProfile {
            date_column: "Posting Date".to_string(),
            date_format: "%m/%d/%Y".to_string(),
            description_column: "Description".to_string(),
            amount_column: Some("Amount".to_string()),
            debit_column: None,
            credit_column: None,
            sign: SignConvention::AsIs,
        },
    );

    // Chase credit card export: Transaction Date,Post Date,Description,
    // Category,Type,Amount,Memo
    // Purchases are negative, payments/credits are positive.
    map.insert(
        "chase-credit".to_string(),
        ImportProfile {
            date_column: "Transaction Date".to_string(),
            date_format: "%m/%d/%Y".to_string(),
            description_column: "Description".to_string(),
            amount_column: Some("Amount".to_string()),
            debit_column: None,
            credit_column: None,
            sign: SignConvention::AsIs,
        },
    );

    // Bank of America checking export: Date,Description,Amount,Running Bal.
    map.insert(
        "bofa".to_string(),
        ImportProfile {
            date_column: "Date".to_string(),
            date_format: "%m/%d/%Y".to_string(),
            description_column: "Description".to_string(),
            amount_column: Some("Amount".to_string()),
            debit_column: None,
            credit_column: None,
            sign: SignConvention::AsIs,
        },
    );

    // Capital One export: Transaction Date,Posted Date,Card No.,
    // Description,Category,Debit,Credit
    // Separate unsigned Debit/Credit columns.
    map.insert(
        "capital-one".to_string(),
        ImportProfile {
            date_column: "Transaction Date".to_string(),
            date_format: "%Y-%m-%d".to_string(),
            description_column: "Description".to_string(),
            amount_column: None,
            debit_column: Some("Debit".to_string()),
            credit_column: Some("Credit".to_string()),
            sign: SignConvention::AsIs,
        },
    );

    // American Express export: Date,Description,Amount
    // Quirk: charges are POSITIVE and payments/credits are NEGATIVE —
    // the opposite of every other profile above — so it must be negated
    // on the way in.
    map.insert(
        "amex".to_string(),
        ImportProfile {
            date_column: "Date".to_string(),
            date_format: "%m/%d/%Y".to_string(),
            description_column: "Description".to_string(),
            amount_column: Some("Amount".to_string()),
            debit_column: None,
            credit_column: None,
            sign: SignConvention::Negated,
        },
    );

    // Generic starter profile users can freely edit for any other bank.
    map.insert(
        "generic".to_string(),
        ImportProfile {
            date_column: "Date".to_string(),
            date_format: "%Y-%m-%d".to_string(),
            description_column: "Description".to_string(),
            amount_column: Some("Amount".to_string()),
            debit_column: None,
            credit_column: None,
            sign: SignConvention::AsIs,
        },
    );

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_name_has_a_profile() {
        let profiles = built_in_profiles();
        for name in BUILT_IN_PROFILE_NAMES {
            assert!(profiles.contains_key(name), "missing profile {name}");
        }
    }

    #[test]
    fn every_built_in_profile_validates() {
        let profiles = built_in_profiles();
        for (name, profile) in &profiles {
            assert!(
                profile.validate(name).is_ok(),
                "profile {name} failed validation"
            );
        }
    }

    #[test]
    fn amex_is_negated_while_others_are_as_is() {
        let profiles = built_in_profiles();
        assert_eq!(profiles["amex"].sign, SignConvention::Negated);
        assert_eq!(profiles["chase-checking"].sign, SignConvention::AsIs);
        assert_eq!(profiles["bofa"].sign, SignConvention::AsIs);
    }

    #[test]
    fn capital_one_uses_debit_credit_columns() {
        let profiles = built_in_profiles();
        let capital_one = &profiles["capital-one"];
        assert!(capital_one.amount_column.is_none());
        assert_eq!(capital_one.debit_column.as_deref(), Some("Debit"));
        assert_eq!(capital_one.credit_column.as_deref(), Some("Credit"));
    }
}
