# Contributing to YOMNOVA

YOMNOVA is an early, experimental project. Small, well-tested changes are the most useful.

## Ground rules
* **Honest status.** Do not describe a feature as working unless a test shows it. Update
  `docs/PROGRESS.md` in the same change.
* **No fake output.** The Playground and docs must show real compiler/VM results only.
* **Fix the compiler, not the examples.** New behaviour needs a test in
  `yomnova/crates/yomnova-compiler/tests/regression.rs` (end to end) or a unit test in the crate.
* Keep the CLI and compiler crates free of third-party dependencies unless there is a strong reason.

## Setup
```bash
# Rust toolchain (stable) and Node.js >= 20
cargo build --release --manifest-path yomnova/Cargo.toml
npm install
npm run dev          # http://localhost:3000, uses yomnova/target/{release,debug}/yomnova
```

## Checks before opening a PR
```bash
cd yomnova
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd .. && npm run lint && npm run build && npm run test:ai   # test:ai needs the compiler built (or YOMNOVA_BIN)
```

## Layout
`yomnova/crates/*` compiler crates · `yomnova/examples` runnable samples (served to the Playground) ·
`src/` web app · `server.ts` API · `docs/` documentation · `public/brand` + `scripts/generate-brand.py` logos.

Licensed under MIT OR Apache-2.0; contributions are accepted under the same terms.
