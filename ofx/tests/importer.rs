//! The importer on whole files: made-up downloads, and real ones when there are some to hand.
//!
//! Real downloads are never committed. To check against yours, put them in `current_account/`
//! and `credit_card/` folders and run
//!
//!     OFX_TEST_DIR=/path/to/downloads cargo test -p ofx
//!
//! Without `OFX_TEST_DIR` those tests pass without checking anything, and say so.

use std::path::Path;
use std::str::FromStr;

use ofx::Ofx;
use pennychest_plugin::{Decimal, Importer, NaiveDate, Row};

/// An OFX 1 file's start, with what identifies the bank.
fn header(bank: &str) -> String {
    format!(
        "OFXHEADER:100
DATA:OFXSGML
VERSION:102
SECURITY:NONE
ENCODING:USASCII
CHARSET:1252
COMPRESSION:NONE
OLDFILEUID:NONE
NEWFILEUID:NONE

<OFX>
    <SIGNONMSGSRSV1>
        <SONRS>
            <STATUS>
                <CODE>0</CODE>
                <SEVERITY>INFO</SEVERITY>
            </STATUS>
            <DTSERVER>20251227101500</DTSERVER>
            <LANGUAGE>ENG</LANGUAGE>
            {bank}
        </SONRS>
    </SIGNONMSGSRSV1>
"
    )
}

const NAMED: &str = "<FI><ORG>Example Bank</ORG></FI>";
const UNNAMED: &str = "<INTU.BID>99999</INTU.BID>";

/// A transaction: its type, date, amount, FITID, name and memo.
type T<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, &'a str);

fn transactions(rows: &[T]) -> String {
    rows.iter()
        .map(|(kind, date, amount, fitid, name, memo)| {
            format!(
                "                    <STMTTRN>
                    \t<TRNTYPE>{kind}</TRNTYPE>
                        <DTPOSTED>{date}000000</DTPOSTED>
                        <TRNAMT>{amount}</TRNAMT>
                        <FITID>{fitid}</FITID>
                        <NAME>{name}</NAME>
                        <MEMO>{memo}</MEMO>
                    </STMTTRN>
"
            )
        })
        .collect()
}

fn bank_statement(bank_id: &str, account_id: &str, kind: &str, as_of: &str, rows: &[T]) -> String {
    let transactions = transactions(rows);
    format!(
        "        <STMTTRNRS>
            <TRNUID>1</TRNUID>
            <STMTRS>
                <CURDEF>GBP</CURDEF>
                <BANKACCTFROM>
                    <BANKID>{bank_id}</BANKID>
                    <ACCTID>{account_id}</ACCTID>
                    <ACCTTYPE>{kind}</ACCTTYPE>
                </BANKACCTFROM>
                <BANKTRANLIST>
                    <DTSTART>20251127000000</DTSTART>
                    <DTEND>20251226000000</DTEND>
{transactions}                </BANKTRANLIST>
                <LEDGERBAL>
                    <BALAMT>3220.00</BALAMT>
                    <DTASOF>{as_of}101500</DTASOF>
                </LEDGERBAL>
            </STMTRS>
        </STMTTRNRS>
"
    )
}

fn bank_file(bank: &str, statement: &str) -> String {
    format!(
        "{}    <BANKMSGSRSV1>\n{statement}    </BANKMSGSRSV1>\n</OFX>\n",
        header(bank)
    )
}

fn card_file(bank: &str, rows: &[T]) -> String {
    let transactions = transactions(rows);
    format!(
        "{}    <CREDITCARDMSGSRSV1>
        <CCSTMTTRNRS>
            <TRNUID>1</TRNUID>
            <CCSTMTRS>
                <CURDEF>GBP</CURDEF>
                <CCACCTFROM>
                    <ACCTID>1234-5678-9012-9010</ACCTID>
                </CCACCTFROM>
                <BANKTRANLIST>
                    <DTSTART>20251129000000</DTSTART>
                    <DTEND>20251212000000</DTEND>
{transactions}                </BANKTRANLIST>
                <LEDGERBAL>
                    <BALAMT>1000.00</BALAMT>
                    <DTASOF>20251212101500</DTASOF>
                </LEDGERBAL>
            </CCSTMTRS>
        </CCSTMTTRNRS>
    </CREDITCARDMSGSRSV1>
</OFX>
",
        header(bank)
    )
}

const CURRENT: [T; 4] = [
    (
        "OTHER",
        "20251201",
        "2500.00",
        "2025120132025335000000000010000",
        "EXAMPLE EMPLOYER",
        "SALARY DEC CR",
    ),
    (
        "OTHER",
        "20251202",
        "-150.00",
        "2025120222025336000000000020000",
        "COUNCIL TAX",
        "DD",
    ),
    (
        "XFER",
        "20251205",
        "-100.00",
        "2025120532025339000000000030000",
        "401122 12345678",
        "INTERNET TRANSFER TFR",
    ),
    (
        "OTHER",
        "20251220",
        "-30.00",
        "2025122022025354000000000040000",
        "J SMITH",
        "FLAT 2 SO",
    ),
];

const CARD: [T; 4] = [
    (
        "DEBIT",
        "20251130",
        "-43.50",
        "MT253350130000010000001",
        "TESCO STORES 2041      LONDON        GBR",
        "",
    ),
    (
        "DEBIT",
        "20251201",
        "-6.95",
        "MT253350130000010000002",
        "CIMOBILE PRET A MANGER LONDON        GBR",
        "",
    ),
    (
        "CREDIT",
        "20251210",
        "312.56",
        "MT253440137000220000003",
        "PAYMENT - THANK YOU",
        "",
    ),
    (
        "DEBIT",
        "20251212",
        "-12.00",
        "MT253460130000010000004",
        "M &amp; S               LONDON        GBR",
        "",
    ),
];

fn current_account() -> String {
    bank_file(
        UNNAMED,
        &bank_statement("401122", "40112231926819", "CHECKING", "20251226", &CURRENT),
    )
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::from_str(text).unwrap()
}

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap()
}

fn summary(rows: &[Row]) -> Vec<(String, String, Decimal)> {
    rows.iter()
        .map(|r| (r.date.to_string(), r.description.clone(), r.amount))
        .collect()
}

fn expected<const N: usize>(rows: [(&str, &str, &str); N]) -> [(String, String, Decimal); N] {
    rows.map(|(d, description, a)| (d.to_owned(), description.to_owned(), dec(a)))
}

#[test]
fn a_current_account() {
    let file = current_account();
    assert!(Ofx.detect(file.as_bytes(), "TransactionHistory.ofx"));

    let info = Ofx.statement_info(file.as_bytes()).unwrap().unwrap();
    assert_eq!(info.statement_type, "current_account");
    // The sort code isn't repeated, though the ACCTID starts with it.
    assert_eq!(info.bank_identifier, "40-11-22 31926819");
    assert_eq!(info.label, "Current Account OFX");
    assert_eq!(info.period_start, Some(date("2025-11-27")));
    assert_eq!(info.period_end, Some(date("2025-12-26")));
    assert_eq!(info.closing_balance, Some(dec("3220.00")));
    assert_eq!(info.opening_balance, Some(dec("1000.00")));

    let rows = Ofx.parse(file.as_bytes()).unwrap();
    assert_eq!(
        summary(&rows),
        expected([
            ("2025-12-01", "EXAMPLE EMPLOYER SALARY DEC CR", "2500.00"),
            ("2025-12-02", "COUNCIL TAX DD", "-150.00"),
            (
                "2025-12-05",
                "401122 12345678 INTERNET TRANSFER TFR",
                "-100.00"
            ),
            ("2025-12-20", "J SMITH FLAT 2 SO", "-30.00"),
        ])
    );
    assert_eq!(rows[0].metadata["fitid"], "2025120132025335000000000010000");
    assert_eq!(rows[0].metadata["transaction_type"], "OTHER");
    assert_eq!(rows[0].line_number, Some(36));
    assert!(
        rows[0]
            .raw_content
            .starts_with("<STMTTRN><TRNTYPE>OTHER</TRNTYPE>")
    );
    assert!(rows[0].raw_content.ends_with("</STMTTRN>"));
}

#[test]
fn a_credit_card() {
    let file = card_file(NAMED, &CARD);
    let info = Ofx.statement_info(file.as_bytes()).unwrap().unwrap();
    assert_eq!(info.statement_type, "credit_card");
    // The last four digits only.
    assert_eq!(info.bank_identifier, "****9010");
    assert_eq!(info.label, "Example Bank Credit Card OFX");
    // Which way round a card's balance is differs between banks.
    assert_eq!(info.closing_balance, None);
    assert_eq!(info.opening_balance, None);

    let rows = Ofx.parse(file.as_bytes()).unwrap();
    assert_eq!(
        summary(&rows),
        expected([
            ("2025-11-30", "TESCO STORES 2041 LONDON GBR", "-43.50"),
            ("2025-12-01", "CIMOBILE PRET A MANGER LONDON GBR", "-6.95"),
            ("2025-12-10", "PAYMENT - THANK YOU", "312.56"),
            ("2025-12-12", "M & S LONDON GBR", "-12.00"),
        ])
    );
}

#[test]
fn a_savings_account() {
    let rows: [T; 2] = [
        (
            "POS",
            "20251201",
            "-12,50",
            "A1",
            "TESCO",
            "CARD PURCHASE DD",
        ),
        // The memo repeats the name.
        (
            "DIRECTDEBIT",
            "20251202",
            "-150.00",
            "A2",
            "COUNCIL TAX",
            "COUNCIL TAX",
        ),
    ];
    let file = bank_file(
        NAMED,
        &bank_statement("201122", "31926819", "SAVINGS", "20251226", &rows),
    );
    assert!(Ofx.detect(file.as_bytes(), "download.qfx"));

    let info = Ofx.statement_info(file.as_bytes()).unwrap().unwrap();
    assert_eq!(info.statement_type, "savings");
    assert_eq!(info.bank_identifier, "20-11-22 31926819");
    assert_eq!(info.label, "Example Bank Savings OFX");
    assert_eq!(info.closing_balance, Some(dec("3220.00")));
    assert_eq!(info.opening_balance, Some(dec("3382.50")));

    let rows = Ofx.parse(file.as_bytes()).unwrap();
    assert_eq!(
        summary(&rows),
        expected([
            // A comma is a decimal point.
            ("2025-12-01", "TESCO CARD PURCHASE DD", "-12.50"),
            ("2025-12-02", "COUNCIL TAX", "-150.00"),
        ])
    );
    assert_eq!(rows[0].metadata["transaction_type"], "POS");
    assert_eq!(rows[0].metadata["fitid"], "A1");
}

#[test]
fn a_balance_dated_after_the_last_day_isnt_read() {
    // It may include transactions the file doesn't have.
    let later = bank_file(
        NAMED,
        &bank_statement("201122", "1", "CHECKING", "20251227", &CURRENT),
    );
    let info = Ofx.statement_info(later.as_bytes()).unwrap().unwrap();
    assert_eq!(info.closing_balance, None);
    assert_eq!(info.opening_balance, None);
}

#[test]
fn a_file_with_several_accounts_says_so() {
    let statement = bank_statement("201122", "1", "CHECKING", "20251226", &CURRENT);
    let file = bank_file(NAMED, &statement.repeat(2));
    assert_eq!(
        Ofx.parse(file.as_bytes()).unwrap_err(),
        "The file has 2 accounts in it. Download each account on its own."
    );
    assert_eq!(Ofx.statement_info(file.as_bytes()).unwrap(), None);
}

#[test]
fn files_it_cant_read_say_so() {
    assert!(!Ofx.detect(b"!Type:Bank\nD01/12/2025\n^", "x.qif"));
    assert_eq!(Ofx.statement_info(b"not ofx").unwrap(), None);
    assert_eq!(
        Ofx.parse(b"not ofx").unwrap_err(),
        "This isn't an OFX file."
    );

    let empty = format!("{}</OFX>", header(NAMED));
    assert_eq!(
        Ofx.parse(empty.as_bytes()).unwrap_err(),
        "The file has no transactions."
    );
    let bad = current_account().replace("<TRNAMT>-150.00", "<TRNAMT>lots");
    assert_eq!(
        Ofx.parse(bad.as_bytes()).unwrap_err(),
        "Transaction 2: \"lots\" isn't an amount."
    );
}

// Real downloads ------------------------------------------------------------------------------

fn check_real(kind: &str) {
    let Some(root) = std::env::var_os("OFX_TEST_DIR") else {
        eprintln!("OFX_TEST_DIR isn't set: no real {kind} downloads checked");
        return;
    };
    let dir = Path::new(&root).join(kind);
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ofx") || e.eq_ignore_ascii_case("qfx"))
        {
            continue;
        }
        let name = path.display();
        let file = std::fs::read(&path).unwrap();
        assert!(Ofx.detect(&file, "download.ofx"), "{name}");
        let info = Ofx.statement_info(&file).unwrap();
        let info = info.unwrap_or_else(|| panic!("{name}: no statement info"));
        assert_eq!(info.statement_type, kind, "{name}");
        if kind == "credit_card" {
            let digits = info.bank_identifier.strip_prefix("****");
            assert!(
                digits.is_some_and(|d| d.len() == 4),
                "{name}: more of the card than its last four digits"
            );
        }
        let rows = Ofx.parse(&file).unwrap();
        assert!(!rows.is_empty(), "{name}: no transactions");
        for (i, row) in rows.iter().enumerate() {
            assert!(
                !row.description.is_empty(),
                "{name} row {i}: no description"
            );
            assert!(!row.amount.is_zero(), "{name} row {i}: zero amount");
            assert!(
                !row.raw_content.is_empty(),
                "{name} row {i}: no raw content"
            );
        }
    }
}

#[test]
fn real_current_account_downloads() {
    check_real("current_account");
}

#[test]
fn real_credit_card_downloads() {
    check_real("credit_card");
}
