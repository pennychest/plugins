# OFX downloads

Imports OFX files into PennyChest. OFX (Open Financial Exchange) is the format many banks offer
as a "Money / Quicken" download, often as `.ofx` or `.qfx`.

- Reads OFX 1 (SGML) and OFX 2 (XML) files with one bank account or credit card in them.
- Reads each transaction's date, description and amount, and keeps the bank's own ID for it.
  Descriptions are as the bank wrote them, with any codes it adds.
- Reads the account and the dates covered. A card shows only its last four digits.
- Reads a bank account's balance when the file says it's for the end of those dates, and works
  out the opening balance from it. Banks differ in how they sign a card's balance, so a card's
  isn't read.

It runs in PennyChest's sandbox: it sees only the file it's sent.

## Testing

```sh
cargo test -p ofx
```

The tests use made-up files. To check against real ones as well, put them in `current_account/`
and `credit_card/` folders somewhere outside the repository and run

```sh
OFX_TEST_DIR=/path/to/downloads cargo test -p ofx
```
