//! Reads an OFX file into a [`Statement`]. OFX 1 is SGML rather than XML: tags needn't be closed,
//! so the file is read as a list of tags and the text after each, which suits OFX 2's XML too.

use pennychest_plugin::{Decimal, NaiveDate};
use std::str::FromStr;

/// Windows-1252's characters for bytes 0x80 to 0x9F; the rest are Latin-1's.
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// The file as text: UTF-8, or else Windows-1252, the usual `CHARSET` of OFX 1.
pub fn decode(file: &[u8]) -> String {
    match std::str::from_utf8(file) {
        Ok(text) => text.to_owned(),
        Err(_) => file
            .iter()
            .map(|&b| match b {
                0x80..=0x9f => CP1252[usize::from(b - 0x80)],
                _ => char::from(b),
            })
            .collect(),
    }
}

/// What an entity stands for: a named one, or a numbered one such as "#39" or "#x27".
fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let code = name.strip_prefix('#')?;
            let number = match code.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            char::from_u32(number)
        }
    }
}

fn unescape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        result.push_str(&rest[..at]);
        let tail = &rest[at..];
        let found = tail
            .find(';')
            .and_then(|end| Some((entity(&tail[1..end])?, end)));
        if let Some((character, end)) = found {
            result.push(character);
            rest = &tail[end + 1..];
        } else {
            // A plain "&", as SGML files can have.
            result.push('&');
            rest = &tail[1..];
        }
    }
    result.push_str(rest);
    result
}

enum Token<'a> {
    /// A tag, the text after it, and where it starts.
    Open {
        name: &'a str,
        value: &'a str,
        at: usize,
    },
    /// A closing tag and where it ends.
    Close { name: &'a str, end: usize },
}

/// The tags from the first `<` on, skipping the header before it and any `<?...>`.
fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while let Some(offset) = text[i..].find('<') {
        let start = i + offset;
        let Some(length) = text[start..].find('>') else {
            break;
        };
        let tag = text[start + 1..start + length].trim();
        let after = start + length + 1;
        let next = text[after..].find('<').map_or(text.len(), |n| after + n);
        i = next;
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        tokens.push(match tag.strip_prefix('/') {
            Some(name) => Token::Close {
                name: name.trim(),
                end: after,
            },
            None => Token::Open {
                name: tag,
                value: text[after..next].trim(),
                at: start,
            },
        });
    }
    tokens
}

#[derive(Debug, Default)]
pub struct Transaction {
    fields: Vec<(String, String)>,
    /// The transaction as the file had it, without the whitespace between tags.
    pub raw: String,
    pub line: usize,
}

impl Transaction {
    /// The field, if it isn't empty.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }
}

/// What an OFX file says, as far as it's read.
#[derive(Debug, Default)]
pub struct Statement {
    /// How many accounts' statements the file holds. The rest describes the first.
    pub statements: usize,
    pub credit_card: bool,
    /// The file's `ORG`: the bank's name, if it gives one.
    pub organisation: Option<String>,
    pub bank_id: Option<String>,
    pub account_id: Option<String>,
    pub account_type: Option<String>,
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub balance: Option<String>,
    pub balance_date: Option<NaiveDate>,
    pub transactions: Vec<Transaction>,
}

/// "20261023000000", "20261023" or "20261023120000.000[0:GMT]": the date is the first eight
/// digits.
pub fn date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d").ok()
}

/// OFX amounts have no thousands separator, and their decimal point can be a comma.
pub fn amount(value: &str) -> Option<Decimal> {
    Decimal::from_str(&value.trim().replace(',', ".")).ok()
}

pub fn read(text: &str) -> Statement {
    let tokens = tokens(text);
    let mut statement = Statement::default();
    let mut open: Vec<&str> = Vec::new();
    let mut current: Option<(Transaction, usize)> = None;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            Token::Open { name, value, at } => {
                // A value, an empty element ("<MEMO></MEMO>"), or an aggregate of others.
                let empty =
                    matches!(tokens.get(i + 1), Some(Token::Close { name: n, .. }) if *n == name);
                if value.is_empty() && !empty {
                    open.push(name);
                    match name {
                        "STMTTRN" => {
                            let line = text[..at].matches('\n').count() + 1;
                            let transaction = Transaction {
                                line,
                                ..Transaction::default()
                            };
                            current = Some((transaction, at));
                        }
                        "STMTRS" | "CCSTMTRS" => {
                            statement.statements += 1;
                            statement.credit_card |=
                                statement.statements == 1 && name == "CCSTMTRS";
                        }
                        _ => {}
                    }
                } else {
                    let value = unescape(value);
                    if let Some((transaction, _)) = &mut current {
                        transaction.fields.push((name.to_owned(), value));
                    } else {
                        read_field(&mut statement, open.last().copied(), name, value);
                    }
                    if empty {
                        i += 1;
                    }
                }
            }
            Token::Close { name, end } => {
                // Elements left open, as SGML allows, close with their aggregate.
                if let Some(depth) = open.iter().rposition(|n| *n == name) {
                    open.truncate(depth);
                }
                if name == "STMTTRN"
                    && let Some((mut transaction, at)) = current.take()
                {
                    transaction.raw = text[at..end].lines().map(str::trim).collect();
                    statement.transactions.push(transaction);
                }
            }
        }
        i += 1;
    }
    statement
}

/// Keeps a value outside the transactions if it's one that's read. The first of each wins, so
/// it's the first account's.
fn read_field(statement: &mut Statement, parent: Option<&str>, name: &str, value: String) {
    let field = match (parent, name) {
        (Some("FI"), "ORG") => &mut statement.organisation,
        (Some("BANKACCTFROM" | "CCACCTFROM"), "BANKID") => &mut statement.bank_id,
        (Some("BANKACCTFROM" | "CCACCTFROM"), "ACCTID") => &mut statement.account_id,
        (Some("BANKACCTFROM"), "ACCTTYPE") => &mut statement.account_type,
        (Some("LEDGERBAL"), "BALAMT") => &mut statement.balance,
        (Some("LEDGERBAL"), "DTASOF") => {
            statement.balance_date = statement.balance_date.or(date(&value));
            return;
        }
        (Some("BANKTRANLIST"), "DTSTART") => {
            statement.start = statement.start.or(date(&value));
            return;
        }
        (Some("BANKTRANLIST"), "DTEND") => {
            statement.end = statement.end.or(date(&value));
            return;
        }
        _ => return,
    };
    field.get_or_insert(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_windows_1252() {
        assert_eq!(decode(b"\xa35 \x80"), "£5 €");
        assert_eq!(decode("£5".as_bytes()), "£5");
    }

    #[test]
    fn reads_entities() {
        assert_eq!(unescape("MORRISON&#39;S"), "MORRISON'S");
        assert_eq!(unescape("A&#x27;B"), "A'B");
        assert_eq!(unescape("B &amp; Q"), "B & Q");
        // Only once, and a plain "&" stays as it is.
        assert_eq!(unescape("&amp;lt;"), "&lt;");
        assert_eq!(unescape("B & Q; M&S"), "B & Q; M&S");
    }

    #[test]
    fn reads_dates_and_amounts() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 23);
        assert_eq!(date("20261023"), day);
        assert_eq!(date("20261023120000.000[0:GMT]"), day);
        assert_eq!(date("2026"), None);
        assert_eq!(amount("-12.50"), Decimal::from_str("-12.50").ok());
        assert_eq!(amount("12,50"), Decimal::from_str("12.50").ok());
        assert_eq!(amount("lots"), None);
    }

    #[test]
    fn reads_sgml_without_closing_tags() {
        let statement = read(
            "OFXHEADER:100\n<OFX><BANKMSGSRSV1><STMTRS><BANKACCTFROM><BANKID>401122\n\
             <ACCTID>31926819<ACCTTYPE>SAVINGS</BANKACCTFROM><BANKTRANLIST>\n\
             <STMTTRN><TRNTYPE>OTHER<DTPOSTED>20251201<TRNAMT>-1.50<NAME>A &amp; B<MEMO>\n\
             </STMTTRN></BANKTRANLIST><LEDGERBAL><BALAMT>10.00<DTASOF>20251202</LEDGERBAL></STMTRS>",
        );
        assert_eq!(statement.statements, 1);
        assert_eq!(statement.bank_id.as_deref(), Some("401122"));
        assert_eq!(statement.account_id.as_deref(), Some("31926819"));
        assert_eq!(statement.account_type.as_deref(), Some("SAVINGS"));
        assert_eq!(statement.balance.as_deref(), Some("10.00"));
        assert_eq!(statement.balance_date, NaiveDate::from_ymd_opt(2025, 12, 2));
        let transaction = &statement.transactions[0];
        assert_eq!(transaction.get("NAME"), Some("A & B"));
        assert_eq!(transaction.get("MEMO"), None);
        assert_eq!(transaction.line, 4);
    }

    #[test]
    fn reads_xml_and_the_bank() {
        let statement = read(
            "<?xml version=\"1.0\"?><?OFX OFXHEADER=\"200\"?><OFX><SIGNONMSGSRSV1><SONRS>\
             <FI><ORG>Example Bank</ORG></FI></SONRS></SIGNONMSGSRSV1>\
             <CREDITCARDMSGSRSV1><CCSTMTTRNRS><CCSTMTRS><CCACCTFROM><ACCTID>1</ACCTID>\
             </CCACCTFROM></CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>",
        );
        assert!(statement.credit_card);
        assert_eq!(statement.organisation.as_deref(), Some("Example Bank"));
    }

    #[test]
    fn a_transfers_other_account_isnt_this_one() {
        let statement = read(
            "<OFX><STMTRS><BANKACCTFROM><ACCTID>1</ACCTID></BANKACCTFROM><BANKTRANLIST><STMTTRN>\
             <BANKACCTTO><ACCTID>2</ACCTID></BANKACCTTO></STMTTRN></BANKTRANLIST></STMTRS></OFX>",
        );
        assert_eq!(statement.account_id.as_deref(), Some("1"));
    }
}
