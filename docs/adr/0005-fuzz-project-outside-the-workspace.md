# 5. The fuzz project lives outside the workspace; seeds are generated

Status: accepted (#17)

## Context
ops/judge/CONTRACT.md §3 requires `fuzz/` to be a cargo-fuzz project and runs every target
nightly; PLAN §6.3 also asks for a short run on every PR. Every cargo-fuzz target links
`libfuzzer-sys`, which:
- contains `unsafe` (CONSTITUTION 8 bans it from the dependency tree, with only the RustCrypto
  allowlist as exception), and
- is licensed `(MIT OR Apache-2.0) AND NCSA` because it bundles LLVM's libFuzzer. NCSA is not
  in the CONSTITUTION 10 list.
Fuzz builds also need nightly and sanitizer flags, which the pinned stable toolchain lacks.
Fuzzing from an empty corpus barely gets past `startxref` (33 coverage edges in 20 s).
CONSTITUTION 18 keeps PDFs out of the repo except minimized repros in `tests/regress/`.

## Decision
1. `fuzz/` is **excluded** from the root workspace and has its own `[workspace]` and
   `Cargo.lock`. `libfuzzer-sys` and its build dependencies never enter the root lockfile,
   so the judge's dependency checks (root `cargo metadata`, `cargo deny`, `cargo vet`) and
   every shipped artifact stay as they are. The judge still scans our fuzz sources for
   `unsafe`, and each target has `#![forbid(unsafe_code)]`.
2. We use `libfuzzer-sys` despite NCSA, as the owner-written CONTRACT §3 requires cargo-fuzz
   and there is no cargo-fuzz target without it. This is the same situation as Playwright's
   bundled licenses (ADR 0002 item 5). ops/proposals/test-only-licenses.md now lists NCSA for
   the owner's decision (#50). If the owner rules against it, the fuzz project must be
   replaced by an owner-approved fuzzer, which needs a CONTRACT change.
3. **Seeds are generated, not committed**: `tools/jpdf-testgen` (a dependency-free workspace
   crate) writes small PDFs with correct xref offsets, and `cargo xtask fuzz` writes them into
   the git-ignored `fuzz/corpus/`.
4. Because the judge's nightly step runs `cargo fuzz run` without our seeds, two targets wrap
   fuzz bytes in a template with a correct xref table (`jpdf_testgen::from_fuzz_chunks`), so
   they reach object parsing and the page tree from an empty corpus (≈ 900 edges in 20 s).
5. Our runs (PR gate and `cargo xtask fuzz`) use a dated nightly (`nightly-2026-11-20`) and
   cargo-fuzz 0.13.2 (`--locked`), and build before running, so a broken nightly release or a
   compile error is reported as a build failure, not a crash. Audits bump the date. The
   judge's nightly step uses plain `nightly`; if that breaks, it is an infra failure there.

## Consequences
- Two lockfiles to refresh in audits (`Cargo.lock`, `fuzz/Cargo.lock`).
- `cargo xtask ci linux` gets slower by about 2 minutes and depends on downloading nightly and
  cargo-fuzz; an outage there is an infra failure (rerun), not a code bug.
- A fuzz crash on a PR blocks that PR even if the PR did not cause it; the fix goes first.
