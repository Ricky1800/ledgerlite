//! `rules.toml` — the categorization rule set and matching engine.

use crate::error::{LedgerError, Result};
use crate::transaction::Transaction;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const RULES_FILE_NAME: &str = "rules.toml";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    /// Case-insensitive substring match against the transaction description.
    Contains,
    /// Regex match against the transaction description.
    Regex,
    /// Matches when the transaction's absolute amount falls within
    /// `[min_amount, max_amount]` (either bound may be omitted).
    AmountRange,
    /// Case-insensitive substring match against the transaction's account
    /// name.
    Account,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub match_type: MatchType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_amount: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_amount: Option<Decimal>,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_c_line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RulesFile {
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

impl RulesFile {
    pub fn load(path: &Path) -> Result<RulesFile> {
        let text = std::fs::read_to_string(path).map_err(|source| LedgerError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| LedgerError::TomlParse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|source| LedgerError::TomlSerialize {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        std::fs::write(path, text).map_err(|source| LedgerError::WriteFile {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// A single compiled rule, ready to be matched against transactions
/// without re-parsing/re-compiling anything per row.
enum CompiledMatcher {
    Contains(String),
    Regex(Regex),
    AmountRange {
        min: Option<Decimal>,
        max: Option<Decimal>,
    },
    Account(String),
}

pub struct CompiledRule {
    matcher: CompiledMatcher,
    pub category: String,
    pub schedule_c_line: Option<String>,
    pub name: String,
}

/// An ordered set of rules, precompiled once and reused across every
/// transaction being categorized.
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
}

impl RuleEngine {
    pub fn compile(rules: &[Rule]) -> Result<RuleEngine> {
        let mut compiled = Vec::with_capacity(rules.len());
        for (index, rule) in rules.iter().enumerate() {
            let matcher = match rule.match_type {
                MatchType::Contains => {
                    let pattern = rule
                        .pattern
                        .clone()
                        .ok_or_else(|| LedgerError::InvalidRule {
                            index,
                            name: rule.name.clone(),
                            reason: "match_type = \"contains\" requires a `pattern`".to_string(),
                        })?;
                    CompiledMatcher::Contains(pattern.to_lowercase())
                }
                MatchType::Regex => {
                    let pattern = rule
                        .pattern
                        .clone()
                        .ok_or_else(|| LedgerError::InvalidRule {
                            index,
                            name: rule.name.clone(),
                            reason: "match_type = \"regex\" requires a `pattern`".to_string(),
                        })?;
                    let compiled_regex =
                        Regex::new(&pattern).map_err(|source| LedgerError::InvalidRegex {
                            index,
                            name: rule.name.clone(),
                            source,
                        })?;
                    CompiledMatcher::Regex(compiled_regex)
                }
                MatchType::AmountRange => {
                    if rule.min_amount.is_none() && rule.max_amount.is_none() {
                        return Err(LedgerError::InvalidRule {
                            index,
                            name: rule.name.clone(),
                            reason:
                                "match_type = \"amount_range\" requires min_amount and/or max_amount"
                                    .to_string(),
                        });
                    }
                    CompiledMatcher::AmountRange {
                        min: rule.min_amount,
                        max: rule.max_amount,
                    }
                }
                MatchType::Account => {
                    let pattern = rule
                        .pattern
                        .clone()
                        .ok_or_else(|| LedgerError::InvalidRule {
                            index,
                            name: rule.name.clone(),
                            reason: "match_type = \"account\" requires a `pattern`".to_string(),
                        })?;
                    CompiledMatcher::Account(pattern.to_lowercase())
                }
            };
            compiled.push(CompiledRule {
                matcher,
                category: rule.category.clone(),
                schedule_c_line: rule.schedule_c_line.clone(),
                name: rule.name.clone(),
            });
        }
        Ok(RuleEngine { rules: compiled })
    }

    /// Returns the first matching rule for `tx`, or `None` if nothing
    /// matches (the transaction should be left/marked Uncategorized).
    pub fn categorize(&self, tx: &Transaction) -> Option<&CompiledRule> {
        let normalized_description = tx.description.to_lowercase();
        let normalized_account = tx.account.to_lowercase();
        self.rules.iter().find(|rule| match &rule.matcher {
            CompiledMatcher::Contains(needle) => normalized_description.contains(needle.as_str()),
            CompiledMatcher::Regex(re) => re.is_match(&tx.description),
            CompiledMatcher::AmountRange { min, max } => {
                let abs = tx.amount.abs();
                let above_min = min.map(|m| abs >= m).unwrap_or(true);
                let below_max = max.map(|m| abs <= m).unwrap_or(true);
                above_min && below_max
            }
            CompiledMatcher::Account(needle) => normalized_account.contains(needle.as_str()),
        })
    }
}

/// Starter rules covering common small-business categories, written out
/// by `ledgerlite init`. Order matters: more specific rules are listed
/// before broad catch-alls.
pub fn starter_rules() -> RulesFile {
    let rule = |name: &str,
                match_type: MatchType,
                pattern: Option<&str>,
                category: &str,
                sched: Option<&str>| Rule {
        name: name.to_string(),
        match_type,
        pattern: pattern.map(|p| p.to_string()),
        min_amount: None,
        max_amount: None,
        category: category.to_string(),
        schedule_c_line: sched.map(|s| s.to_string()),
    };

    RulesFile {
        rules: vec![
            rule(
                "Software subscriptions",
                MatchType::Regex,
                Some("(?i)adobe|microsoft|dropbox|slack|zoom|notion|github|google workspace|quickbooks|figma"),
                "Software",
                Some("Line 27a - Other expenses (Software)"),
            ),
            rule(
                "Advertising",
                MatchType::Regex,
                Some("(?i)facebook ads|meta ads|google ads|instagram ads|adwords|mailchimp|constant contact"),
                "Advertising",
                Some("Line 8 - Advertising"),
            ),
            rule(
                "Fuel",
                MatchType::Regex,
                Some("(?i)shell|exxon|chevron|bp gas|sunoco|gas station|fuel"),
                "Fuel",
                Some("Line 9 - Car and truck expenses"),
            ),
            rule(
                "Meals",
                MatchType::Regex,
                Some("(?i)starbucks|restaurant|cafe|coffee|doordash|grubhub|uber eats"),
                "Meals",
                Some("Line 24b - Meals"),
            ),
            rule(
                "Supplies",
                MatchType::Regex,
                Some("(?i)staples|office depot|amazon|walmart|costco"),
                "Supplies",
                Some("Line 22 - Supplies"),
            ),
            rule(
                "Bank fees",
                MatchType::Regex,
                Some("(?i)monthly service fee|overdraft|maintenance fee|wire fee|nsf fee"),
                "Bank Fees",
                Some("Line 27a - Other expenses (Bank fees)"),
            ),
            rule(
                "Owner draw",
                MatchType::Regex,
                Some("(?i)owner draw|equity draw|distribution to owner"),
                "Owner Draw",
                None,
            ),
            rule(
                "Sales income",
                MatchType::Regex,
                Some("(?i)deposit|stripe payout|square inc|invoice payment|customer payment|sales receipt|zelle from"),
                "Sales Income",
                Some("Line 1 - Gross receipts"),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn tx(description: &str, amount: Decimal, account: &str) -> Transaction {
        Transaction {
            id: "id".into(),
            date: NaiveDate::from_ymd_opt(2026, 9, 2).unwrap(),
            account: account.into(),
            description: description.into(),
            raw_description: description.into(),
            amount,
            category: None,
            schedule_c_line: None,
            source_file: "test.csv".into(),
            occurrence: 0,
            receipt_path: None,
        }
    }

    #[test]
    fn contains_match_is_case_insensitive() {
        let rules = vec![Rule {
            name: "coffee".into(),
            match_type: MatchType::Contains,
            pattern: Some("starbucks".into()),
            min_amount: None,
            max_amount: None,
            category: "Meals".into(),
            schedule_c_line: None,
        }];
        let engine = RuleEngine::compile(&rules).unwrap();
        let matched = engine
            .categorize(&tx("STARBUCKS STORE #123", dec!(-4.50), "Chase Checking"))
            .unwrap();
        assert_eq!(matched.category, "Meals");
    }

    #[test]
    fn regex_match_works() {
        let rules = vec![Rule {
            name: "software".into(),
            match_type: MatchType::Regex,
            pattern: Some("(?i)adobe|figma".into()),
            min_amount: None,
            max_amount: None,
            category: "Software".into(),
            schedule_c_line: None,
        }];
        let engine = RuleEngine::compile(&rules).unwrap();
        assert!(engine
            .categorize(&tx(
                "ADOBE CREATIVE CLOUD",
                dec!(-54.99),
                "Chase Credit Card"
            ))
            .is_some());
        assert!(engine
            .categorize(&tx("NETFLIX", dec!(-15.99), "Chase Credit Card"))
            .is_none());
    }

    #[test]
    fn amount_range_matches_absolute_value() {
        let rules = vec![Rule {
            name: "small fees".into(),
            match_type: MatchType::AmountRange,
            pattern: None,
            min_amount: Some(dec!(0)),
            max_amount: Some(dec!(5)),
            category: "Bank Fees".into(),
            schedule_c_line: None,
        }];
        let engine = RuleEngine::compile(&rules).unwrap();
        assert!(engine
            .categorize(&tx("MONTHLY FEE", dec!(-3.00), "Chase Checking"))
            .is_some());
        assert!(engine
            .categorize(&tx("BIG PURCHASE", dec!(-300.00), "Chase Checking"))
            .is_none());
    }

    #[test]
    fn account_match_works() {
        let rules = vec![Rule {
            name: "credit card only".into(),
            match_type: MatchType::Account,
            pattern: Some("credit".into()),
            min_amount: None,
            max_amount: None,
            category: "Card Spend".into(),
            schedule_c_line: None,
        }];
        let engine = RuleEngine::compile(&rules).unwrap();
        assert!(engine
            .categorize(&tx("ANYTHING", dec!(-10.00), "Chase Credit Card"))
            .is_some());
        assert!(engine
            .categorize(&tx("ANYTHING", dec!(-10.00), "Chase Checking"))
            .is_none());
    }

    #[test]
    fn first_match_wins() {
        let rules = vec![
            Rule {
                name: "specific".into(),
                match_type: MatchType::Contains,
                pattern: Some("starbucks".into()),
                min_amount: None,
                max_amount: None,
                category: "Meals".into(),
                schedule_c_line: None,
            },
            Rule {
                name: "catch all supplies".into(),
                match_type: MatchType::Contains,
                pattern: Some("star".into()),
                min_amount: None,
                max_amount: None,
                category: "Supplies".into(),
                schedule_c_line: None,
            },
        ];
        let engine = RuleEngine::compile(&rules).unwrap();
        let matched = engine
            .categorize(&tx("STARBUCKS STORE #1", dec!(-4.50), "Chase Checking"))
            .unwrap();
        assert_eq!(matched.category, "Meals");
    }

    #[test]
    fn no_match_returns_none() {
        let rules = vec![Rule {
            name: "coffee".into(),
            match_type: MatchType::Contains,
            pattern: Some("starbucks".into()),
            min_amount: None,
            max_amount: None,
            category: "Meals".into(),
            schedule_c_line: None,
        }];
        let engine = RuleEngine::compile(&rules).unwrap();
        assert!(engine
            .categorize(&tx("MYSTERY VENDOR", dec!(-4.50), "Chase Checking"))
            .is_none());
    }

    #[test]
    fn compile_rejects_regex_rule_without_pattern() {
        let rules = vec![Rule {
            name: "broken".into(),
            match_type: MatchType::Regex,
            pattern: None,
            min_amount: None,
            max_amount: None,
            category: "X".into(),
            schedule_c_line: None,
        }];
        assert!(RuleEngine::compile(&rules).is_err());
    }

    #[test]
    fn compile_rejects_invalid_regex() {
        let rules = vec![Rule {
            name: "broken regex".into(),
            match_type: MatchType::Regex,
            pattern: Some("(unclosed".into()),
            min_amount: None,
            max_amount: None,
            category: "X".into(),
            schedule_c_line: None,
        }];
        assert!(RuleEngine::compile(&rules).is_err());
    }

    #[test]
    fn starter_rules_all_compile() {
        let file = starter_rules();
        assert!(RuleEngine::compile(&file.rules).is_ok());
        assert!(!file.rules.is_empty());
    }
}
