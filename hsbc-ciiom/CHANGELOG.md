# Changelog

## 0.3.0

- Renamed from `hsbc`, as it reads HSBC CIIOM statements. If you have `hsbc` installed, install
  this and uninstall that.
- Fixes amounts of about £1,000 or more on cards and £10,000 or more on current accounts, and
  balances of £10,000 or more, being misread: transactions could go missing, or come in with the
  wrong amount or direction.
- Fixes transactions going missing when a line's date sits a little lower than the rest of it.
- Reads balances with a stray space in them.
- Adds a card's interest as a transaction on the statement's date.
- Checks each statement adds up, and imports nothing from one that doesn't.

## 0.2.0

- Imports HSBC PDF statements: current accounts, savings and credit cards, with the account,
  period and balances read from the statement. Runs as a WebAssembly module in PennyChest's
  sandbox.
