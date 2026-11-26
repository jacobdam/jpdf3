# Principles: how jpdf is built

Each entry is an invariant of the product's code (P4: of its `cargo xtask` tooling), with the ADR that decided it, how it is
enforced and what a reviewer checks (ADR 0011 §6). The CONSTITUTION's rules apply everywhere
and are not repeated here; `ops/LESSONS.md` is about how the agent works, not the product.

**Who reads it.** A Worker loads the entries whose `Paths:` cover the files it changes. The
reviewer brief (ops/TICK.md §4) holds the entries whose `Paths:` match a file in the diff. A
Design: technical session (ops/TICK.md §8) loads all of them, and adds or changes entries only
through a design ADR (`docs/adr/_TEMPLATE-design.md`), never in a Worker PR. Until `area:`
labels exist (ADR 0011 §4, #246), `Paths:` is how an entry is matched to a change: each item
is a git `:(glob)` pathspec from the repository root (`**` crosses directories), and a path
ending in `/` covers everything under it. `git diff --name-only origin/main...HEAD --
':(glob)<path>'` lists the changed files an item matches.

**Concerns with no principle yet** (a spec that touches one files a `design` issue instead of
improvising, ADR 0011 §6): the memory budget (#236), isolation of the engine from the apps
(#223).

## P1. Callers can tighten limits, never loosen them
- **Invariant:** every value that reaches parsing through `jpdf_engine::Document::open` is at
  most the `Limits::default()` value of its field, whatever the front end (Swift, JS, CLI)
  passed. Bindings enter only through `jpdf-engine`; the layers below it take `&Limits`
  unclamped and are called only by the engine, by tests and by the fuzz targets (`fuzz/`, which
  build their own `Limits`).
- **Decided in:** ADR 0008.
- **Enforced by:** `Document::open` calls `Limits::clamped_to(&Limits::default())` before using
  any field (crates/jpdf-engine/src/lib.rs); `clamped_to` destructures `Limits` exhaustively, so
  a new field fails to compile until it is listed (types), and the `all_fields` tests in
  crates/jpdf-traits/src/limits.rs check that each listed field is clamped.
- **Reviewers check:** a new `Limits` field is in `clamped_to` and in the tests' `all_fields`
  literal; a new public entry point that takes `Limits` from a front end clamps it the same way;
  no binding calls a layer below `jpdf-engine`.
- **Paths:** `crates/jpdf-traits/src/limits.rs`, `crates/jpdf-engine/`, `tools/jpdf-cli/`, `bindings/`.

## P2. Backends see per-call bounds and return one error type
- **Invariant:** no method of a `jpdf-traits` backend trait takes `Limits`. A method that bounds
  work on its input takes the small `Copy` struct derived for that call (`DecodeLimits` from
  `Limits::decode_limits`, `RasterLimits`, `FontLimits`), and every fallible method returns
  `BackendError`. The engine alone keeps document-wide accounting; callers map `BackendError`
  into their own error enums.
- **Decided in:** ADR 0004.
- **Enforced by:** the trait signatures in `crates/jpdf-traits/src/` (types), and the
  conformance suites under `crates/jpdf-traits/src/conformance/`, which check the
  `BackendError` variant.
- **Reviewers check:** a new or changed trait method takes the per-call struct, not `Limits` or
  a raw remaining-budget integer the backend could misuse; it returns `BackendError`; the caller,
  not the backend, charges the document-wide budget.
- **Paths:** `crates/jpdf-traits/`, `crates/jpdf-backends/`, and the callers that derive the
  per-call structs and charge the budget: `crates/jpdf-engine/`, `crates/jpdf-filters/`,
  `crates/jpdf-crypt/`, `crates/jpdf-font/`, `crates/jpdf-render/`.

## P3. Third-party code enters only through `jpdf-backends`
- **Invariant:** root workspace members other than `jpdf-backends` depend on no third-party
  crate in their normal or build dependencies; third-party functionality is reached through a
  `jpdf-traits` trait implemented in `jpdf-backends` (CONSTITUTION 11). The fuzz project is
  outside the root workspace with its own lockfile, so `libfuzzer-sys` never enters the root
  lockfile.
- **Decided in:** CONSTITUTION 11; ADR 0005 (fuzz).
- **Enforced by:** reviewer vigilance for the "only `jpdf-backends`" part: no gate checks it
  today. The judge's dependency checks (`cargo deny`, `cargo vet`, protected) check licenses,
  audits and `unsafe` of whatever is added, not which crate adds it; the jpdf-cli dependency
  gate (specs/tooling/cli-dependency-gate.md) checks only the `jpdf-cli` graph. The root
  `Cargo.toml` has `exclude = ["fuzz"]`.
- **Reviewers check:** a `Cargo.toml` change outside `crates/jpdf-backends/` adds no
  third-party normal or build dependency; `fuzz/` stays excluded.
- **Paths:** `**/Cargo.toml`.

## P4. A multi-step tool reports each step; one driver decides the order
- **Invariant:** a `cargo xtask` command whose spec fixes an order of side-effecting steps, or
  runs a step after another failed, records each step's result as a `steps::Outcome` (`Ok`,
  `Finding`, `Infra`, `Failed`) under the step's name; one driver function, calling the steps
  through an injected interface, decides which run and in what order and returns the log; the
  exit code is `steps::exit_code(&log)` (`Failed` 1, else `Infra` 75, else `Finding` 4, else 0).
  A tool whose every step stops it on the first failure, in source order, needs no driver.
  The nightly doesn't hold it yet (see Enforced by).
- **Decided in:** ADR 0013.
- **Enforced by:** not yet: ADR 0013's tasks (designed in #357) add the types in
  `xtask/src/steps.rs` (the driver returns the log; steps return an `Outcome`) and each
  driver's tests with a recording fake (the full order, one run per failing step, the
  `exit_code` table); the real adapters and `run`'s wiring stay reviewer vigilance. Until
  then the nightly folds its results in `with_limits`, `exit_code` and `extra::run`'s
  `bool`; `cargo xtask ci linux` stays the known exception until a PR changes its checks
  (ADR 0013, Requirements on future callers).
- **Reviewers check:** a new or moved step is called by the driver, not by `run` or another
  step; its real adapter maps every error of the function it wraps to `Failed` or `Infra` and
  decides nothing about order; no step's result is folded into a `bool` or an exit code before
  the end; the driver's tests cover the new step's failure.
- **Paths:** `xtask/src/`.
