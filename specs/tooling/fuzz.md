# Fuzz infrastructure (`fuzz/`, `cargo xtask fuzz`, jpdf-testgen)

Area: tooling
ISO 32000-2 clauses: §7.5.2 (header), §7.5.4–§7.5.6 (xref, trailer, incremental updates) for the generated seeds; none for the tooling itself
Journeys: none (development tool; PLAN §6.3, R1; ops/judge/CONTRACT.md §3)
Status: implemented

## Behavior
1. **`fuzz/`** is a cargo-fuzz project (issue #17). It is excluded from the root workspace
   (`exclude = ["fuzz"]`), has its own `[workspace]` and its own `fuzz/Cargo.lock`, and builds
   only on nightly. The judge runs every target from
   `cargo fuzz list` nightly for up to 5 minutes each with `-rss_limit_mb=2048` (CONTRACT §3).
2. **Targets** (each `#![forbid(unsafe_code)]`; the judge scans their text for `unsafe`):
   | target | input | reaches |
   |---|---|---|
   | `engine_open` | raw bytes into `Document::open(data, &Limits::default())`, the path behind `jpdf-cli info` | header/`startxref` search, xref, trailer, page tree |
   | `engine_objects` | bytes split by 0xFF (`FF 00` = a literal 0xFF byte, so bodies can hold any byte): chunk 0 = extra trailer entries, chunks 1… = object bodies numbered from 1 (max 64; later chunks ignored), wrapped by `jpdf_testgen::from_fuzz_chunks` into a file whose xref table is always correct | object parser, dictionaries, `/Prev` and trailer handling, page tree, from an empty corpus |
   | `syntax_resolve` | the same input both raw and through the template; `XrefDocument::parse`, then `resolve` on `/Root` and on object numbers 0–255 twice | `resolve` on in-use, free, missing and re-read objects and overlap checks (template half); mismatched headers and generations only through the raw half, which mutates the `engine_open` seeds |
   A `cli_args` target is not added: argument parsing lives in the `jpdf-cli` binary and is
   covered by `tools/jpdf-cli/tests/cli.rs`; a target needs a library entry point (later, if
   the parser grows).
3. **Seeds.** `tools/jpdf-testgen` (workspace crate, no dependencies) writes small PDFs whose
   xref offsets are correct by construction: 0/1/3-page files, a 2-update incremental file, a
   nested page tree, and a truncated file (`engine_open`); three chunk-format inputs
   (`engine_objects`); `syntax_resolve` gets both sets. The same generator writes the test files
   of `crates/jpdf-engine/tests/open.rs` and `tools/jpdf-cli/tests/cli.rs` (a dev-dependency of
   jpdf-engine and jpdf-cli, #86) and the bench files (a normal dependency of the bench, xtask
   and the fuzz targets, none of them shipped), so those test files no longer carry their own copy of it. Other
   writers remain: engine tests that need hand-placed or broken sections (incremental updates,
   `/Prev` chains, overlapping objects or sections) write those files or sections inline, and
   the unit tests in `crates/jpdf-syntax/src/xref.rs` keep a private writer (`file`,
   `with_xref`). Seeds are generated, never committed: CONSTITUTION 18
   keeps PDFs out of the repo except minimized repros in `tests/regress/`. `fuzz/corpus/` and
   `fuzz/artifacts/` are git-ignored.
4. **`cargo xtask fuzz [--short] [--seconds N] [--target NAME]`** writes the seeds into
   `fuzz/corpus/<target>/seed-<name>` (overwriting old seed files, keeping what the fuzzer
   found), builds all targets (`cargo fuzz build --debug-assertions`; a build failure is
   reported as such, not as a crash), then runs each listed target with
   `--debug-assertions -- -max_total_time=N -rss_limit_mb=2048 -timeout=10`. Default N = 300
   (as the judge), `--short` = 30, `--seconds` takes 1–3600. Exit 0 when no target fails,
   1 when the build fails or any target crashes, hangs or runs out of memory (the message
   names the targets and how to minimize), 2 with the usage line on a usage error.
   All runs use the dated toolchain `nightly-2026-11-20` (`xtask/src/fuzz.rs` `NIGHTLY`,
   bumped in audits like `rust-toolchain.toml`), so a broken nightly release cannot turn PRs
   red; the judge's own nightly step uses plain `nightly`.
5. **`cargo xtask ci linux`** runs the `--short` run for every target after the web build and
   check-specs (PLAN §6.3: "run on every PR (short)"). It first fails fast, installing
   nothing, if `fuzz/Cargo.toml` is missing. With `CI` set and the tools missing it installs
   the pinned nightly (`rustup toolchain install nightly-2026-11-20 --profile minimal`) and
   cargo-fuzz 0.13.2 (`cargo install --locked --version`). Locally a missing tool fails with
   the install hint. It also runs `cargo fmt --manifest-path fuzz/Cargo.toml --check`,
   because `cargo fmt --all` does not see the excluded project.
6. **Rule for new code** (`fuzz/README.md`): every new parser, filter, backend adapter and
   engine command handler adds a target and its seeds (`xtask/src/fuzz.rs` `seeds_for`); a
   unit test fails if a target in `fuzz/Cargo.toml` has no seeds.
7. **Crashes.** A crash artifact is minimized (`cargo +nightly fuzz tmin`) and committed as a
   repro under `tests/regress/` by the PR that fixes it, with a test that opens it.

## Persistence
Nothing persists in the repo. The local corpus in `fuzz/corpus/` grows across runs; CI starts
from the generated seeds each time.

## Undo
Not applicable (tooling).

## Errors
- No `fuzz/Cargo.toml`: `xtask: fuzz: ./fuzz/Cargo.toml not found (run from the repository root)`, exit 1.
- Missing nightly or cargo-fuzz: `xtask: fuzz: `cargo +nightly-2026-11-20 fuzz list` failed (install: …)`, exit 1.
- Targets do not compile: `fuzz targets do not build with nightly-2026-11-20 (a build failure, not a crash)`, exit 1.
- `fuzz/` not formatted (`ci linux` only): exit 1.
- Unknown `--target`: lists the known targets, exit 1.
- Bad arguments: the problem and the usage line, exit 2 (`--seconds` outside 1–3600, unknown flag, missing value).
- Crash/hang/OOM: `crash, hang or out-of-memory in [targets]; artifacts are in fuzz/artifacts/ …`, exit 1.

## Limits
Targets use `Limits::default()`, the same as `jpdf-cli`, so a finding is a real-world bug.
The run limits mirror the judge: 2048 MB RSS; 10 s per input (the judge uses libFuzzer's
default 1200 s, so our gate is stricter); inputs up to libFuzzer's default 4096 bytes.
`from_fuzz_chunks` writes at most 64 objects; its output is linear in the input size.
Overflow checks (`[profile.release]` in `fuzz/Cargo.toml`) and debug assertions are on.

## Security
- No `unsafe` in our fuzz sources (CONSTITUTION 3; `forbid(unsafe_code)` compiles with
  `fuzz_target!`). `libfuzzer-sys` (needed by every cargo-fuzz target) contains `unsafe` and
  bundles LLVM libFuzzer under NCSA, which CONSTITUTION 10 allows only for test and build
  tools (amended for #50); it is only in `fuzz/Cargo.lock`, never in the root lockfile or
  any shipped artifact. ADR 0005 item 2 records the decision.
- Seeds are our own generated bytes, never corpus files (CONSTITUTION 13, 18).

## Accessibility
Not applicable (tooling).

## Platforms
macOS (arm64) and Linux (x86_64) with a nightly toolchain. The engine code fuzzed is the same
on every platform.

## Performance
`--short` adds about 90 s of fuzzing plus the one-off nightly build to `ci linux`; on CI the
first install of cargo-fuzz takes about 1–2 minutes more. Locally, a 20 s run reaches ≈ 900
coverage edges on the template targets from an empty corpus versus 33 for `engine_open`
without seeds (≈ 1150 with seeds).

## Tests
- `tools/jpdf-testgen/tests/seeds.rs`: every xref entry points at its object; minimal files
  open with their page counts; each `engine_open` seed opens with its expected result
  (truncated → `Malformed`, nested tree → `Unsupported` until nested trees land); template
  seeds open; chunk encoding round-trips, including escaped 0xFF; the template caps objects;
  2000 deterministic pseudo-random inputs (up to 4 KiB, dense in 0xFF and PDF tokens) give
  correct offsets and open without panicking, raw and through the template.
- `xtask/src/fuzz.rs` unit tests: argument parsing (bounds, unknown flags), every target in
  `fuzz/Cargo.toml` has seeds, seed writing keeps fuzzer-found files.
- `xtask/tests/check_specs.rs`: `fuzz` usage errors exit 2; `ci linux` in a tree without
  `fuzz/` ends with the fuzz error line and exits 1.
- `cargo fuzz list` (as the judge runs it) lists the three targets; `cargo xtask fuzz --short` runs them.
