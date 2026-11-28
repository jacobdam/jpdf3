# Principles: how jpdf is built

Each entry is an invariant of the product's code or its `cargo xtask` tooling, with the ADR
that decided it, how it is enforced and what a reviewer checks (ADR 0011 §6). The
CONSTITUTION's rules apply everywhere and are not repeated here; `ops/LESSONS.md` is about how
the agent works, not the product.

**Who reads it.** A Worker loads the entries whose `Paths:` cover the files it changes. The
reviewer brief (ops/TICK.md §4) holds the entries whose `Paths:` match a file in the diff. A
Design: technical session (ops/TICK.md §8) loads all of them, and adds or changes entries only
through a design ADR (`docs/adr/_TEMPLATE-design.md`), never in a Worker PR. Until `area:`
labels exist (ADR 0011 §4, #246), `Paths:` is how an entry is matched to a change: each item
is a git `:(glob)` pathspec from the repository root (`**` crosses directories), and a path
ending in `/` covers everything under it. `git diff --name-only origin/main...HEAD --
':(glob)<path>'` lists the changed files an item matches.

**Concerns with no principle yet** (a spec that touches one files a `design` issue instead of
improvising, ADR 0011 §6): isolation of the engine from the apps (#223).

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
  `Finding` is only a result the spec gives exit 4 (a failed limits case); a measured result the
  spec records without failing the tool (a regress `crash`, a bench `timeout`, a fuzz `crash`)
  is `Ok`.
  A tool whose every step stops it on the first failure, in source order, needs no driver.
- **Decided in:** ADR 0013.
- **Enforced by:** the types in `xtask/src/steps.rs` (the driver returns the log; steps return
  an `Outcome`; `exit_code` and its table test) and each driver's tests with a recording fake
  (the full order, one run per failing step); for the nightly, `drive` in
  `xtask/src/nightly.rs` and `xtask/src/nightly/tests/driver.rs` (ADR 0013 task 1, #360). The
  real adapters and `run`'s wiring stay reviewer vigilance. The nightly holds it since ADR 0013
  task 2 (#361: the bench, the fuzz pass and `nightly.json` are steps of their own), and
  `cargo xtask ci linux` since #369 (`drive` in `xtask/src/ci_linux.rs` and its tests;
  specs/tooling/ci-linux.md), which ADR 0013's Requirements on future callers had left as the
  known exception.
- **Reviewers check:** a new or moved step is called by the driver, not by `run` or another
  step; its real adapter maps every error of the function it wraps to `Failed` or `Infra` as
  the spec's exit codes say, and decides nothing about order; no step's result is folded into
  a `bool` or an exit code before the end; the driver's tests cover the new step's failure.
- **Paths:** `xtask/src/`.

## P5. A protected file's tests run against the file, on its platform, from one manifest
- **Invariant:** every test of a protected file (CONSTITUTION 12) is an entry in
  `ops/protected-tests/manifest.toml` with its script, targets, platforms, tools and helper
  files; each target matches `ops/judge/protected.txt` and exists. The runner passes each
  target's protected path to the script, or a copy's path when told to test a copy, and never
  writes a file at a protected path. An entry's platforms include every platform where a
  target runs (macOS for `ops/driver/`: the Mac mini and the hosted `macos-15` runner; Linux
  for a workflow job on `ubuntu-*`); it passes on main on each, and a missing tool fails it. A
  PR that changes `ops/protected-tests/` is in the strict lane. An applied proposal's
  `ops/proposals/<topic>/` directory holds no test and no copy of a protected file.
- **Decided in:** ADR 0014.
- **Enforced by:** reviewer vigilance until ADR 0014's tasks land: today the tests still sit in
  the applied proposals' directories and nothing runs them. Then `cargo xtask protected-tests`
  (task 2: a failing entry, a missing tool, an unlisted file, a missing or unprotected target,
  a target or platform the platform table rejects) in `cargo xtask ci linux` for the Linux
  entries (task 3) and `cargo xtask ci macos` for the macOS ones (task 4). "Applied" has no
  gate.
- **Reviewers check:** until ADR 0014 task 2 has merged, only that a changed test of a
  protected file still takes its target as an argument and the cases it adds or
  changes pass on the target's platform.
  From then on: a new or changed test of a protected file has a manifest entry with the
  target's platforms and helpers, and takes the protected path from the runner; the runner
  never writes a protected path. From ADR 0014 task 5 on (the proposal life cycle): a proposal
  PR ran the runner with its copy's path and says which entries fail on purpose; a test it
  changes stays in its own directory and its apply steps move it into `ops/protected-tests/`
  in the same signed commit; an applied proposal's directory keeps only its `.md` and `.diff`
  records.
- **Paths:** `ops/protected-tests/`, `ops/proposals/`, `xtask/src/protected_tests*`.

## P6. The Planner gates `ready`, at most three times a UTC day, and reruns only for a new issue
- **Invariant:** agent sessions add `ready` only in a Planner session, except for the design
  and product queues (`design` plus `ready`, `design-product` plus `ready`, ops/TICK.md §9's
  opening). `cargo xtask next-session` picks the Planner only while fewer than 3 distinct
  journal files dated today (UTC) hold a `- Session: planner…` record, and on a day that
  already has one, only at step 11 (nothing before it applies) and only when an issue is
  *new*: open, in the current milestone, without `ready`, a skip label, `in-progress`,
  `design`, `design-pending` or `design-product`, not in flight, and created at or after the
  end of the minute in the newest such record's file name (`HHMM-`; a name without a valid
  time reads as the end of its day).
- **Decided in:** ADR 0015 (amending ADR 0011 §1's once-a-day bound).
- **Enforced by:** `cargo xtask next-session` (xtask/src/next_session.rs) and its `--from`
  fixtures (xtask/tests/next_session.rs), from ADR 0015 task 1 on (#440). That a
  session filing Worker issues leaves `ready` off is reviewer vigilance.
- **Reviewers check:** a change to step 8 or 11 keeps the count per distinct file (one file
  on two refs counts once), compares an issue's creation time to the newest record's minute
  (not its day, not the oldest record), and keeps the rerun last, after step 10; each filter of
  the *new* test has a near-miss fixture; no title or body of a new issue is printed (numbers
  only); a session's TICK text that files Worker issues doesn't add `ready`.
- **Paths:** `xtask/src/next_session.rs`, `xtask/src/next_session/`,
  `xtask/tests/next_session.rs`, `specs/tooling/next-session.md`, `ops/TICK.md`.

## P7. One memory cap at the allocator, one charging path, measured
- **Invariant:** in a process that holds the engine, the bytes Rust's global allocator has
  handed out never pass the hard cap `H` (1536 MiB, or lower; 1024 MiB in the fuzz targets),
  counted by `jpdf_alloc::CappedAlloc`. In the non-test code of the crates under `crates/` and
  of `tools/jpdf-cli` (the `cargo xtask mem-lint` scope, ADR 0017 §3), every allocation whose
  size is a runtime value goes through `jpdf-mem`, which refuses it with a typed error
  (`LimitExceeded("memory")`, CLI exit 2) once the counted bytes plus reservations plus the
  request would pass the soft cap `S = H − 128 MiB`; code that allocates on its own (a
  third-party decoder) runs only inside an `admit` reservation of its per-call limits. Every
  limits-harness case and replayed fuzz input ends with exit 0 or 2, its counted peak within
  its baseline (`ops/metrics/limits-peaks.json`, per platform and `--mib`), on Linux its OS
  peak under 2 GiB − 64 MiB, and on macOS its footprint under 2 GiB − 64 MiB and within its
  baseline. The safety of the budget is never argued from a model of allocator behavior.
- **Decided in:** ADR 0017.
- **Enforced by:** nothing yet; ADR 0017's tasks add it. Then: `jpdf-mem`'s fallible API
  (types, task 1); `cargo xtask mem-lint` in `cargo xtask ci linux` (task 6); `CappedAlloc`'s
  refusal at `H` (task 7, after the owner applies the CONSTITUTION 3 exception, tasks 2 and 3);
  the limits harness in `cargo xtask ci linux` at `--mib 64` on every PR, in the nightly at
  500 MiB, and on macOS in `cargo xtask ci macos` (pushes to main only), and the nightly fuzz
  replay (tasks 8 to 10). Checkpoints in input loops, reservations around backend calls, copies
  through trait methods the lint can't list, and the allocator's installation in each binary
  stay reviewer vigilance. Until task 8 is merged, decode and raster stay off the jpdf-cli path
  (specs/core/memory-budget.md, Gate; specs/tooling/cli-dependency-gate.md).
- **Reviewers check:** an allocation in the gated code goes through `jpdf-mem` (from task 6 the
  lint sees the std methods it lists; a method it misses is still a finding); input-sized data
  isn't copied through `clone`, `to_owned`, `From`/`Into` or `extend` outside `jpdf-mem`; each
  loop over input that allocates calls `checkpoint()`; a backend call runs inside a reservation
  of its output plus working bytes; a binary or fuzz target that holds the engine installs
  `CappedAlloc` and its gauge; no cap, soft-cap margin, ceiling or baseline goes up without the
  PR saying why (CONSTITUTION 14). **The process rule:** a memory finding about allocator
  behavior is a blocker or major only with a concrete input and one of those violations, or a
  measurement (a harness or fuzz case that fails). A predicted allocator behavior
  (fragmentation, arenas, retention, rounding) with no measurement is a minor whose fix is a new
  harness case, never a new model term or default. ADR 0010's other grades still apply as they
  are: a false statement, or a missing test for a behavior or limit the PR claims (an
  exact-boundary test of `S`, say), is a major. A finding about code that doesn't exist yet is
  answered by ADR 0017's Requirements on future callers or by the "Requirements on future code"
  list in specs/core/memory-budget.md (which exists today and which ADR 0017 task 11 keeps):
  disputed as covered by one of them, or added to that spec list in the same PR; no issue is
  filed for it.
- **Paths:** `crates/`, `tools/jpdf-cli/`, `fuzz/`, `bindings/`, `apps/macos/JPDFHelper/`,
  `xtask/src/limits.rs`, `xtask/src/limits/`, `specs/core/memory-budget.md`.
