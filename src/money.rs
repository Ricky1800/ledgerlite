//! Money parsing helpers.
//!
//! `rust_decimal::Decimal` is used everywhere money is represented — never
//! `f64` — so that cents never go missing to floating point rounding.
//! Bank CSV exports are inconsistent about how they format negative
//! amounts, so this module normalizes the common conventions:
//!
//! - Plain signed numbers: `-45.00`, `+12.50`
//! - Accounting-style parentheses for negatives: `(45.00)`
//! - Trailing minus sign: `45.00-`
//! - Currency symbols and thousands separators: `$1,234.56`

use rust_decimal::Decimal;
use std::str::FromStr;

/// Parses a money string into a [`Decimal`], tolerating the formatting
/// quirks common in bank/credit-card CSV exports.
pub fn parse_money(raw: &str) -> Result<Decimal, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("amount is empty".to_string());
    }

    let mut negative = false;
    let mut s = trimmed.to_string();

    // Accounting notation: (123.45) means -123.45
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 2 {
        negative = true;
        s = s[1..s.len() - 1].to_string();
    }

    // Strip currency symbols, thousands separators, and stray whitespace.
    s = s
        .chars()
        .filter(|c| !matches!(c, '$' | ',' | ' ' | '\u{a0}'))
        .collect();

    // Trailing minus sign convention: "123.45-"
    if let Some(stripped) = s.strip_suffix('-') {
        negative = true;
        s = stripped.to_string();
    }

    // Leading plus sign is meaningless to Decimal::from_str only in the
    // sense that we want to allow it explicitly; Decimal actually accepts
    // a leading '+' already, so this is just defensive.
    if let Some(stripped) = s.strip_prefix('+') {
        s = stripped.to_string();
    }

    if s.is_empty() {
        return Err(format!("'{raw}' has no digits"));
    }

    let value = Decimal::from_str(&s).map_err(|e| format!("'{raw}' is not a valid number: {e}"))?;

    Ok(if negative { -value.abs() } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_plain_positive() {
        assert_eq!(parse_money("45.00").unwrap(), dec!(45.00));
    }

    #[test]
    fn parses_plain_negative() {
        assert_eq!(parse_money("-45.00").unwrap(), dec!(-45.00));
    }

    #[test]
    fn parses_leading_plus() {
        assert_eq!(parse_money("+45.00").unwrap(), dec!(45.00));
    }

    #[test]
    fn parses_parentheses_as_negative() {
        assert_eq!(parse_money("(45.00)").unwrap(), dec!(-45.00));
    }

    #[test]
    fn parses_trailing_minus() {
        assert_eq!(parse_money("45.00-").unwrap(), dec!(-45.00));
    }

    #[test]
    fn parses_currency_symbol_and_thousands_separator() {
        assert_eq!(parse_money("$1,234.56").unwrap(), dec!(1234.56));
    }

    #[test]
    fn parses_negative_currency_with_thousands() {
        assert_eq!(parse_money("-$1,234.56").unwrap(), dec!(-1234.56));
    }

    #[test]
    fn parses_parenthesized_currency() {
        assert_eq!(parse_money("($1,234.56)").unwrap(), dec!(-1234.56));
    }

    #[test]
    fn rejects_empty_string() {
        assert!(parse_money("").is_err());
        assert!(parse_money("   ").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_money("not-a-number").is_err());
    }

    #[test]
    fn never_loses_cents_to_float_rounding() {
        // 0.1 + 0.2 famously != 0.3 in f64. Decimal must get this exact.
        let a = parse_money("0.10").unwrap();
        let b = parse_money("0.20").unwrap();
        assert_eq!(a + b, dec!(0.30));
    }
}
