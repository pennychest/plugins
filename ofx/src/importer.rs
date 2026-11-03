//! Turns what [`crate::reader`] reads into PennyChest's rows and statement details, from OFX's
//! standard fields alone.

use pennychest_plugin::{Decimal, Importer, ImporterInfo, Row, StatementInfo};
use serde_json::{Map, Value};

use crate::reader::{self, Statement, Transaction};

fn rows(statement: &Statement) -> Result<Vec<Row>, String> {
    if statement.statements > 1 {
        return Err(format!(
            "The file has {} accounts in it. Download each account on its own.",
            statement.statements
        ));
    }
    if statement.transactions.is_empty() {
        return Err("The file has no transactions.".into());
    }
    statement
        .transactions
        .iter()
        .enumerate()
        .map(|(i, t)| row(t).map_err(|e| format!("Transaction {}: {e}", i + 1)))
        .collect()
}

fn row(transaction: &Transaction) -> Result<Row, String> {
    let posted = transaction
        .get("DTPOSTED")
        .ok_or("it has no date (DTPOSTED).")?;
    let date = reader::date(posted).ok_or_else(|| format!("\"{posted}\" isn't a date."))?;
    let amount_text = transaction
        .get("TRNAMT")
        .ok_or("it has no amount (TRNAMT).")?;
    let amount =
        reader::amount(amount_text).ok_or_else(|| format!("\"{amount_text}\" isn't an amount."))?;

    let memo = transaction.get("MEMO").unwrap_or_default();
    let name = transaction.get("NAME").unwrap_or_default();
    // Some banks repeat the name as the memo.
    let memo = if memo == name { "" } else { memo };
    let description = format!("{name} {memo}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut metadata = Map::new();
    let mut keep = |key: &str, value: &str| {
        metadata.insert(key.into(), Value::String(value.into()));
    };
    if let Some(kind) = transaction.get("TRNTYPE") {
        keep("transaction_type", kind);
    }
    if let Some(fitid) = transaction.get("FITID") {
        keep("fitid", fitid);
    }
    Ok(Row {
        date,
        description,
        amount,
        raw_content: transaction.raw.clone(),
        line_number: i64::try_from(transaction.line).ok(),
        metadata,
    })
}

/// "12-34-56 12345678" for a sort code, the bank's ID and account otherwise, or "****1234" for a
/// card, whose full number is never kept.
fn bank_identifier(statement: &Statement) -> Option<String> {
    let account = statement.account_id.as_deref()?;
    if statement.credit_card {
        let digits: Vec<char> = account.chars().filter(char::is_ascii_digit).collect();
        let last_four: String = digits.get(digits.len().checked_sub(4)?..)?.iter().collect();
        return Some(format!("****{last_four}"));
    }
    let Some(bank_id) = statement.bank_id.as_deref() else {
        return Some(account.to_owned());
    };
    let account = account.strip_prefix(bank_id).unwrap_or(account);
    Some(
        if bank_id.len() == 6 && bank_id.chars().all(|c| c.is_ascii_digit()) {
            format!(
                "{}-{}-{} {account}",
                &bank_id[..2],
                &bank_id[2..4],
                &bank_id[4..]
            )
        } else {
            format!("{bank_id} {account}")
        },
    )
}

/// The balance at the end of the file's dates: a bank account's, if it's dated the last day, as a
/// later one may include transactions the file doesn't have. A card's isn't read, as banks differ
/// in how they sign it.
fn closing_balance(statement: &Statement) -> Option<Decimal> {
    if statement.credit_card || statement.end.is_none() || statement.balance_date != statement.end {
        return None;
    }
    reader::amount(statement.balance.as_deref()?)
}

pub struct Ofx;

impl Importer for Ofx {
    fn info(&self) -> ImporterInfo {
        ImporterInfo::new("ofx", "OFX downloads", &["ofx", "qfx"])
            .describe("Bank account and credit card transactions, downloaded as OFX")
            .detects()
            .with_statement_info()
    }

    /// OFX files say so at the start, whatever they're called.
    fn detect(&self, file: &[u8], _filename: &str) -> bool {
        let start = String::from_utf8_lossy(&file[..file.len().min(4096)]);
        start.contains("OFXHEADER") || start.contains("<OFX>")
    }

    fn parse(&self, file: &[u8]) -> Result<Vec<Row>, String> {
        let text = reader::decode(file);
        if !text.contains("<OFX>") {
            return Err("This isn't an OFX file.".into());
        }
        rows(&reader::read(&text))
    }

    fn statement_info(&self, file: &[u8]) -> Result<Option<StatementInfo>, String> {
        let statement = reader::read(&reader::decode(file));
        if statement.statements != 1 {
            return Ok(None);
        }
        let Some(bank_identifier) = bank_identifier(&statement) else {
            return Ok(None);
        };
        let (statement_type, kind) = if statement.credit_card {
            ("credit_card", "Credit Card")
        } else if matches!(
            statement.account_type.as_deref(),
            Some("SAVINGS" | "MONEYMRKT")
        ) {
            ("savings", "Savings")
        } else {
            ("current_account", "Current Account")
        };
        let closing_balance = closing_balance(&statement);
        let opening_balance = closing_balance
            .zip(rows(&statement).ok())
            .map(|(c, rows)| c - rows.iter().map(|r| r.amount).sum::<Decimal>());
        Ok(Some(StatementInfo {
            statement_type: statement_type.into(),
            bank_identifier,
            label: match &statement.organisation {
                Some(name) => format!("{name} {kind} OFX"),
                None => format!("{kind} OFX"),
            },
            name_hint: None,
            period_start: statement.start,
            period_end: statement.end,
            opening_balance,
            closing_balance,
        }))
    }
}
