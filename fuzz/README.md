# fuzz/

cargo-fuzz targets for jpdf. Spec: [specs/tooling/fuzz.md](../specs/tooling/fuzz.md).
The judge runs every target nightly (ops/judge/CONTRACT.md §3); `cargo xtask ci linux` runs a
30-second pass per target on every PR.

```sh
rustup toolchain install nightly-2026-11-20 --profile minimal   # xtask/src/fuzz.rs NIGHTLY
cargo install cargo-fuzz --locked --version 0.13.2
cargo xtask fuzz --short                    # seeds + 30 s per target
cargo xtask fuzz --target engine_objects    # one target, 300 s
```

## Rules
- **Every new parser, filter, backend adapter and engine command handler adds a target** here
  (a `[[bin]]` in `Cargo.toml`) and its seeds in `xtask/src/fuzz.rs` (`seeds_for`). A unit test
  fails if a target has no seeds.
- Targets call the code with `Limits::default()` and ignore the result: only a panic, abort,
  hang (> 10 s per input) or more than 2048 MB RSS is a finding.
- Each target starts with `#![forbid(unsafe_code)]`; the judge scans every `.rs` file.
- Seeds come from `tools/jpdf-testgen`, never from corpus files; `corpus/` and `artifacts/`
  are git-ignored (CONSTITUTION 13, 18).
- A crash is fixed by a PR that adds the minimized input (`cargo +nightly-2026-11-20 fuzz tmin
  <target> <artifact>`) to `tests/regress/` with a test that opens it.
- This directory has its own `Cargo.lock`; `libfuzzer-sys` stays out of the root lockfile
  (ADR 0005).
