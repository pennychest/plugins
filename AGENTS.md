# AGENTS.md

Notes for coding agents working in this repository. The [README](README.md) is the full guide for
people; this is the short version, plus the rules that aren't obvious from the code.

## What's here

Official plugins for [PennyChest](https://github.com/pennychest/pennychest). Each top-level
directory (`hsbc/`, `beancount/`) is a Rust crate in one Cargo workspace, built as a WebAssembly
(WASI) module that PennyChest runs in a sandbox. Each has its own version, `CHANGELOG.md` and
tests.

- `pennychest-plugin.toml` in each plugin: the manifest PennyChest reads (name, version, kinds,
  the `.wasm` to run)
- `plugins.json`: what PennyChest's **Settings → Plugins** reads to list and install plugins
- `tools/package.sh`: builds a plugin and packages it in `dist/` as Settings → Plugins installs it
- `tools/update_index.py`: points a plugin's `plugins.json` entry at a release
- `.github/workflows/release.yml`: releases a plugin when its tag is pushed

Plugins implement the `pennychest-plugin` SDK, which lives in the PennyChest repository and is
pinned to a revision in the workspace `Cargo.toml`. What a plugin can do is described in
[the plugin protocol](https://pennychest.github.io/plugins/protocol/). If a change needs something
new from the SDK, it belongs in a pull request to pennychest first; then move the pinned `rev`.

## Setup and checks

`rust-toolchain.toml` pins the toolchain, with rustfmt, clippy and the `wasm32-wasip1` target, so
rustup installs what's needed. Run what CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked
cargo test --locked
cargo build --release --locked --target wasm32-wasip1
```

CI sets `RUSTFLAGS=-D warnings`, so a warning fails it.

## Rules

- **Versions and changelogs are edited by hand.** A plugin's version is in both its `Cargo.toml`
  and its `pennychest-plugin.toml`, and they must match; `tools/package.sh` refuses to package
  otherwise. Add a section to its `CHANGELOG.md` for each version: the newest section becomes the
  release notes.
- **Releases are tags.** `main` only takes pull requests, so bump the version and changelog in a
  pull request, then tag the merged commit `<plugin>-v<version>` and push the tag. The Release
  workflow builds and packages the plugin, attaches it to a GitHub release, and opens a pull
  request updating `plugins.json`. Don't edit `plugins.json` by hand.
- **Commit titles start with a gitmoji:** ✨ features, 🐛 fixes, 📝 docs, 👷 CI, ✅ tests, 🔖
  releases. Keep each commit to one plugin where you can.
- **No real financial data in the repository.** The HSBC tests use made-up statements from
  `hsbc/tests/fixtures/make_statements.py`; regenerate the PDFs with it rather than adding new
  ones. Real statements are read from `HSBC_TEST_PDF_DIR` (or kept in the gitignored
  `statements/`), and tests on them assert structure, never values. Don't commit statements, text
  extracted from them or values from them.
- **Keep plugins small.** They're downloaded when installed, which is why the release profile
  optimises for size. Think twice before adding a dependency, and `unsafe` is forbidden.
- **READMEs are published.** Changes to `README.md` or `*/README.md` on `main` rebuild
  pennychest.github.io, so write them for users. A plugin README's first line is its name in
  Settings → Plugins.

## Adding a plugin

1. Add `<name>/`, modelled on `hsbc/` (importer) or `beancount/` (exporter): a `Cargo.toml` that
   uses the workspace's `pennychest-plugin` dependency and lints, a `pennychest-plugin.toml` at
   the same version, a `README.md`, a `CHANGELOG.md` and tests.
2. Add it to `members` in the workspace `Cargo.toml`, and to the table in `README.md`.
3. Once it's merged, tag `<name>-v<version>`. Its `plugins.json` entry is added by the release.
