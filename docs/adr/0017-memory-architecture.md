# 0017. The memory budget is predicted by a model, so every review finds another uncounted case

Status: proposed (design issue #236, an owner issue; queued by the owner in #238's first design
queue). A design ADR merges 24 hours after the latest of: its PR marked ready, its last
`ai-review` success, the owner's last `/release`, unless the owner comments `/hold` (ADR 0011 §7;
ops/TICK.md §8 step 5). The owner's `/approve` ends the window early.

## The problem class
The judge runs `jpdf-cli` under a 2 GiB `RLIMIT_AS` (ops/judge/CONTRACT.md); running out is a
crash. specs/core/memory-budget.md answers "does a hostile file fit?" by **predicting** the peak:
`jpdf_syntax::MemoryBudget` adds persistent terms (file, retained, xref table × 3, spans × 64 bytes,
page walk × 48), the largest phase twice (parse at 272 bytes per object, decode, raster) and a
256 MiB reserve whose items are placeholders. The spec itself calls the result "not a proven
bound", and only the model's own tests use it. The group:

- **#109** (closed): the four charged budgets summed to exactly 2 GiB. It added the model and
  lowered four defaults.
- **#146** (open, `sev:high`, M0): glibc keeps a finished parse's heap mapped next to a decode's
  new mappings, so the largest phase counts twice; leftovers of several phases still aren't
  bounded ("can't be bounded by a formula over the limits", the spec's own words). It lowered six
  more defaults.
- **#154** (closed): the gate that keeps decode and raster off the jpdf-cli path until #146
  closes, enforced on the package graph (`tools/jpdf-cli/tests/dependency_gate.rs`).
- **#156** (open, M10): what evidence closes #146; non-main glibc arenas reserve 64 MiB
  sub-heaps unevenly.
- **#165** (open, M10): macOS libmalloc keeps freed parse buffers; probes measured 1.24 GB and
  1.65 GB where the model's terms gave about 0.6 GB.
- **#168** (closed): JBIG2 page data and globals both alive exceed the decode term; it added
  `ImageInputBudget`, a shared allowance for an image's inputs.
- **#187** (closed): the allowance must charge capacity, not length, including the copy that
  gives spare capacity back (`charge_output`).
- **#231** (open, deferred, `sev:high`, M2): the future image-decode caller must use
  `charge_output`, and every adapter must keep its buffer capacity within the cap.
- Related, not instances: **#141** (the CLI's file read can double while the file grows) and
  **#114** (sub-limit structs bypass the clamp; the engine thread's stack).

The ledgers: PR #142 (6 rounds), PR #151 (15 rounds; the owner stopped it), PR #160 (audit
0003), PR #186 and PR #232. In #151 every second-reviewer major was "not proven" (ADR 0010).

**The class:** a safety property of the whole process (the address space stays under 2 GiB) is
checked by arithmetic over a model of code and allocator behavior, instead of being enforced
where memory is handed out and measured where it is used. Each term is an estimate about code
we write (growth factors, node sizes) or code we don't control (glibc's brk heap, mmap threshold
and arenas; macOS libmalloc), so a reviewer can always name one more case.

## Why point fixes don't converge
- #109 added the terms the reviews named. PR #142's R3.1 found that a freed phase isn't given
  back (#146). That isn't a missing term but a wrong assumption, so the fix counted the largest
  phase twice and lowered six defaults.
- #146's fix covers one earlier phase. Round 15 of PR #151 found that one passing workload
  doesn't bound other histories (#156), and that glibc arenas don't behave like the reserve's
  one 64 MiB item. Audit 0003 measured macOS retaining far more than the model (#165).
- #168 found two inputs that each fit their cap but not the term together. The fix added an
  allowance; #187 found that the allowance must count capacity and the shrink copy; #231 carries
  what is left to a caller that doesn't exist yet.

Each fix made the model larger and the defaults smaller, and each new term opened a new place
to be wrong. **The missing decision:** where the budget is enforced. Today nothing in the
process counts or refuses memory; the model only argues that the limits make running out
unlikely. An enforced cap makes the model unnecessary: the question changes from "is the sum of
estimates under 2 GiB?" to "does every allocation go through the one counter, and does the
measured peak stay under the line?", which a test and a measurement can answer.

## Decision

### 1. One counter and one hard cap, at the global allocator
- A new crate `crates/jpdf-alloc` holds `Capped`, a `GlobalAlloc` that wraps `std::alloc::System`
  and counts the bytes it hands out (`layout.size()` on alloc and alloc_zeroed, the difference on
  realloc, subtracted on dealloc) in an atomic, with a peak. It refuses (returns null) any
  request that would take the count past the **hard cap** `H`. `jpdf-cli` installs it with
  `#[global_allocator]`, and so do the fuzz targets, the macOS engine helper and the web worker
  once they hold the engine (Requirements on future callers).
- **CONSTITUTION 3 doesn't allow it today.** `GlobalAlloc` is an `unsafe trait`: no safe
  implementation exists, and a crate that provides one would bring `unsafe` into the dependency
  tree, which CONSTITUTION 8 forbids. So task 2 proposes one more sole exception, beside
  `bindings/`: `crates/jpdf-alloc`, at most about 150 lines, holding only the `GlobalAlloc` impl
  over `System` and its counters (no FFI, no other unsafe code), with the judge's policy check
  (ops/judge/policy.py) changed to match. The owner applies it (task 3); tasks 7 to 11 wait for
  it. If the owner declines, the counter can't exist and this ADR is reopened (a new `design`
  issue), not patched.
- **`H` starts at 1536 MiB** (1.5 GiB). The 512 MiB below 2 GiB is for what the counter doesn't
  see: the binary and its libraries, thread stacks, the C allocator's own overhead, arenas and
  fragmentation. That margin is not modelled: task 8 measures it (the OS peak minus the counted
  peak) on every harness case and fails when the OS peak passes 2 GiB − 64 MiB, and the fix for
  a failure is a lower `H` (always allowed, CONSTITUTION 4) or a structural change, never a term.
- A front end may lower the cap (`JPDF_MEM_CAP_MIB` for `jpdf-cli`, for tests and the harness),
  never raise it, as P1 does for `Limits`.

### 2. One fallible charging path: `jpdf-mem`
- A new crate `crates/jpdf-mem` (no `unsafe`) is the only place in the engine's library code
  that allocates a buffer whose size is a runtime value. It offers fallible growth
  (`try_with_capacity`, `try_reserve`, `try_push`, `try_extend_from_slice`, `try_filled`,
  `try_box`, `try_arc`, a fallible B-tree and hash map insert, `try_collect`; the exact set is
  what the migrated code needs) and two checks for code it can't wrap:
  - `admit(bytes)`: room for a bounded call into code that allocates on its own (a third-party
    decoder in `jpdf-backends`, bounded by its per-call limits, P2), returning a `Reservation`
    that holds the bytes until dropped;
  - `checkpoint()`: called in every loop over input that allocates.
- Every helper first checks the **soft cap** `S = H − 128 MiB` against a `Gauge` (the counted
  bytes plus live reservations plus the request), then allocates with `try_reserve` where std
  has a fallible API; structures with none (B-tree nodes, `Box`, `Arc`) are admitted first and
  then allocated. The gauge is a trait in `jpdf-mem` that `jpdf-alloc` implements and the binary
  installs once; tests install a fake gauge, so the soft-cap boundary is tested exactly without
  an allocator. With no gauge installed (before task 7, or a binary that doesn't install one) the
  check passes and only `try_reserve` can refuse.
- **At the cap the result is a typed error, never an abort.** A refused charge is
  `jpdf_mem::MemError`, which each crate maps into its own `LimitExceeded("memory")` (backends:
  `BackendError::LimitExceeded("memory")`). `jpdf-cli` prints `jpdf-cli: limit exceeded: memory`
  and exits 2. The 128 MiB between `S` and `H` is for allocations that don't pass through the
  path (std internals, a backend that overruns what it was admitted for, a race between
  threads), so the allocator's own refusal at `H` is reached only by a bug. That refusal makes an
  infallible allocation abort (std's `handle_alloc_error`; the hook that would change it,
  `set_alloc_error_hook`, is unstable, and the judge requires a stable toolchain); an abort is a
  crash, which the harness and the fuzzers report, and each one found is a `crash` issue.
- The per-call work limits of `Limits` stay as they are (P1, P2): they bound the work and the
  output of one call and keep a single object or stream sane. They no longer need to sum to
  2 GiB, and this ADR raises none of them (CONSTITUTION 4).

### 3. Mechanical enforcement: `cargo xtask mem-lint`
A gate in `cargo xtask ci linux` runs clippy's `disallowed_methods` over the **library code**
(`--lib`, so `#[cfg(test)]` code isn't compiled) of every crate under `crates/` and of
`tools/jpdf-cli`, except `jpdf-mem` and `jpdf-alloc`, with its own configuration
(`CLIPPY_CONF_DIR`), at `-D clippy::disallowed_methods`. The list denies the std APIs that
allocate a runtime-sized or growing buffer: the sized constructors, reserve and resize methods,
`vec![x; n]`, the growth methods of `Vec`, `VecDeque`, `String`, the hash and B-tree maps and
sets, slice and `str` copies and repeats (`to_vec`, `repeat`, `concat`, `join`), `collect` into
a collection, `Box::new`, `Arc::new`, `Rc::new`, `format!` and `to_string`. It works on resolved
paths, not text, so a renamed import or a re-export doesn't bypass it (PR #151: the owner
rejected text scanners). Each entry has a fixture that the gate must reject. It doesn't touch the
workspace's clippy run or the protected root `clippy.toml`, and no code may `allow` it
(CONSTITUTION 5).

### 4. Verification by measurement
- `cargo xtask limits` (#25) records, for each case, the counted peak (`JPDF_MEM_REPORT=1` makes
  `jpdf-cli` print it to stderr at exit) and the OS peak (`VmPeak` from `/proc/self/status`,
  printed by `jpdf-cli` on Linux; on macOS, the `peak memory footprint` that `/usr/bin/time -l`
  reports). On Linux the runs stay under `ulimit -v 2097152`.
- A case fails (exit 4, as a limits finding does) when its counted peak passes the committed
  baseline `ops/metrics/limits-peaks.json` by more than 10% plus 1 MiB, when its OS peak passes
  2 GiB − 64 MiB, or when the counted peak is 0 (no allocator installed). The counted peak of
  single-threaded code is the same on every run, so the baseline can be tight; the OS peak is
  noisy, so it gets only the ceiling. Raising a baseline is like weakening an assertion: the PR
  says why (CONSTITUTION 14).
- New cases: `memory-cap` (a lowered `JPDF_MEM_CAP_MIB` that a valid file passes, expecting exit
  2 `limit exceeded: memory`), and #146's history (many cached growing containers, then
  requests of growing sizes; the parse-only part today, the decode part with the first decode
  phase). The same cases run on macOS in `cargo xtask ci macos` (the `project-macos` job) at
  `--mib 64`, where `RLIMIT_AS` isn't enforced and the counter is the cap.
- The fuzz targets install `Capped` at `H`, so an allocation past the cap is a refused charge or
  a saved crash. The nightly also replays each target's corpus with a `--sanitizer none` build
  under `ulimit -v 2097152`: AddressSanitizer reserves terabytes of shadow address space at
  start, so a sanitizer build can't run under a real `RLIMIT_AS`.
- specs/core/memory-budget.md is rewritten (task 11) as the cap, the charging rule, the
  measurement and the living list of requirements on future code. `MemoryBudget`, the glibc
  chunk model (`alloc_bytes`; the resolve cache then charges requested capacities),
  `ImageInputBudget` and `charge_output` are deleted: they existed to make the sum fit.

### 5. Alternatives considered
- **Hand out memory from one region reserved up front** (the owner's question). It would make
  glibc and macOS behave the same and put fragmentation under our control. It is rejected for
  now: it means writing a general allocator (size classes, free lists, realloc, alignment,
  threads) in hand-written `unsafe`, many times the exception asked for in §1, and fragmentation
  inside our own region would still exist: a hostile pattern would fail at the region's end
  instead of at `RLIMIT_AS`. A vetted third-party allocator brings `unsafe` into the tree
  (CONSTITUTION 8). If the measurement shows the OS peak passing the margin on a case that a
  lower `H` can't fix, a new `design` issue takes this up with that case as evidence.
- **Tune glibc** (a fixed mmap threshold, one arena with `mallopt`): it needs FFI, so it needs
  the exception too, and it targets the effects the measurement now catches. Revisit with a
  failing case, as above.
- **Keep the model and only measure:** the model is what didn't converge; the measurement alone
  can't turn "out of memory" into a typed error.
- **Rely on `RLIMIT_AS` and `try_reserve` alone (no exception):** large buffers would fail
  cleanly on Linux, but nothing would cap the macOS helper, which has no `RLIMIT_AS`, and small
  growing allocations would still abort. Tasks 1, 4, 5 and 6 are this level; the allocator adds
  the cap.

## The invariant
In a process that holds the engine (`jpdf-cli`; later the macOS helper, the web worker, the fuzz
targets):
1. the bytes Rust's global allocator has handed out never pass the hard cap `H` (1536 MiB, or
   lower), counted by `jpdf_alloc::Capped`;
2. in the library code of the engine crates and `jpdf-cli`, every allocation whose size is a
   runtime value goes through `jpdf-mem`, which refuses it with a typed error once the counted
   bytes plus reservations plus the request would pass the soft cap `S = H − 128 MiB`; code
   that allocates on its own (a third-party decoder) runs only inside an `admit` reservation of
   its per-call limits;
3. on every case of the limits harness and every replayed fuzz input, the run ends with exit 0
   or 2, never an abort, its counted peak is within its baseline, and on Linux its OS peak stays
   under 2 GiB − 64 MiB.

A reviewer can say "this diff breaks it" for: an allocation in the gated code that isn't through
`jpdf-mem`, an input loop with no checkpoint, a backend call outside a reservation, a binary that
holds the engine without installing `Capped`, a raised cap or baseline, or a harness case whose
peak got worse.

## Enforcement
1. **Types:** `jpdf-mem`'s helpers return `Result`, so a refused charge can't be ignored
   silently; `Reservation` releases on drop; the gauge is installed once. `Capped` refuses past
   `H` whatever the caller does. Types can't stop library code from calling `Vec::push` instead
   of the helper: that is the lint's job.
2. **A lint or gate:** `cargo xtask mem-lint` (§3) in `cargo xtask ci linux`, which the judge's
   `project-linux` job runs, so it needs no protected change. The CONSTITUTION exception and the
   policy check are protected: task 2's proposal. The jpdf-cli dependency gate's allowed set
   gains `jpdf-mem` and `jpdf-alloc` in the tasks that add them.
3. **A measurement:** the limits harness on Linux and macOS and the fuzz replay (§4), with the
   baseline and the OS ceiling.
4. **Reviewer vigilance:** P7 in docs/PRINCIPLES.md: that each input loop has its checkpoint,
   that a backend call is inside a reservation, that a binary installs the allocator, and the
   process rule for findings (below). These are the parts no lint sees.

## Verification
- `jpdf-alloc`'s tests count alloc, alloc_zeroed, realloc up and down, and dealloc exactly, and
  refuse at `H` + 1 byte while `H` passes. Still passing: an allocator that counts right but
  forgets `alloc_zeroed` in the cap check is caught by the zeroed case; one that is never
  installed is caught by the harness's "counted peak is 0" rule.
- `jpdf-mem`'s tests, with a fake gauge: a charge that reaches `S` exactly passes and one byte
  more is refused; a reservation counts until dropped; sums saturate. Still passing: a helper
  that checks the cap but then allocates infallibly. The lint doesn't cover `jpdf-mem` itself, so
  its tests must reach each helper's refusal through the gauge and its `try_reserve` failure
  through a request of `isize::MAX` bytes.
- The lint's fixtures: one per denied method, including through `vec!` and `format!`. Still
  passing: a std method that allocates but isn't on the list. The list is checked against std's
  `alloc` API when the toolchain is updated (audit sessions), and the measurement catches what it
  misses.
- The harness: a `jpdf-cli` without the allocator fails (counted peak 0); one that leaks or
  grows fails its baseline; one that aborts at the cap is a `crash`; one that ignores
  `JPDF_MEM_CAP_MIB` fails `memory-cap`. Still passing: a hostile allocation history that no
  case reproduces. The cases come from findings (§ the process rule) and the fuzzers explore the
  rest; this is the gap that stays open, and it is a missing test case, not a missing term.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #109 | kept closed; its model is deleted by task 11 | the lowered defaults stay (CONSTITUTION 4); the arithmetic goes |
| #146 | folded into task 8, whose PR closes it; `deferred` until then | the cross-phase history becomes a harness case under the real limit, measured instead of bounded by a formula; the gate stays until then |
| #154 | kept closed; the gate stays | it keeps decode and raster off the jpdf-cli path until task 8 is merged and the first decode phase's PR passes its own harness cases (Requirements on future callers); task 8 restates the lift condition in specs/tooling/cli-dependency-gate.md |
| #156 | superseded (closed with a link to this ADR) | what closes #146 is decided here (task 8's cases under the cap); arena reservations are part of the OS peak the harness measures |
| #165 | folded into task 9, whose PR closes it | the macOS measurement runs #165's re-resolve probe as a case, and the counter caps the live bytes there |
| #168 | kept closed; `ImageInputBudget` is deleted by task 11 | a JBIG2 decode whose inputs and working memory don't fit under `S` fails with `LimitExceeded("memory")`; the per-stream cap stays |
| #187 | kept closed; `charge_output` is deleted by task 11 | capacity is what the allocator counts, so there is nothing to give back by copying |
| #231 | superseded (closed with a link to this ADR) | the image caller allocates through `jpdf-mem` (no `charge_output`); its item 3, the adapters' output capacity at the exact cap, is a P2 per-call limit tested by the conformance suites, and is requirement 4 below |
| #141 | kept (sev:medium, M10) | the cap turns its 3 × growth into a refused charge or a cap abort, but the read itself should stop at the size + 1 byte |
| #114 | kept (sev:medium, M10) | the clamp is P1's; thread stacks are outside the counter and inside the measured margin |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `crates/jpdf-mem`: the helpers, `admit`/`Reservation`, `checkpoint`, the `Gauge` trait with a one-time install, the soft cap and `MemError`; tests with a fake gauge at the exact soft-cap boundary, reservation release and saturation; spec `specs/core/memory-charging.md`. About 700 lines. | #236 |
| 2 | `ops/proposals/allocator-exception.md`: CONSTITUTION 3's second sole exception (`crates/jpdf-alloc`, its limits as in §1) and the matching ops/judge/policy.py change, with a test plan. About 300 lines. | #236 |
| 3 | Owner: apply task 2's proposal (signed commits). Filed without `ready`; `owner-todo` once task 2's PR merges (ops/TICK.md §0). | task 2 |
| 4 | Migrate jpdf-syntax onto `jpdf-mem` (lexer, parser containers, xref table, spans, trailer, resolve cache), checkpoints in its input loops, `LimitExceeded("memory")`; boundary tests with a fake gauge (a parse that crosses `S` fails with the error, never panics); `jpdf-mem` added to the jpdf-cli dependency gate. About 800 lines. | task 1 |
| 5 | Migrate jpdf-cos, jpdf-engine and jpdf-cli (`read_capped` through `jpdf-mem`) the same way, with their boundary tests. About 500 lines. | task 4 |
| 6 | `cargo xtask mem-lint` (§3) in `cargo xtask ci linux`: the configuration, one fixture per denied method, the crate list, spec `specs/tooling/mem-lint.md`. About 600 lines. | task 5 |
| 7 | `crates/jpdf-alloc`: `Capped` (count, peak, refusal at `H`), its `Gauge` impl; installed in jpdf-cli with `JPDF_MEM_CAP_MIB` (lower only) and `JPDF_MEM_REPORT` (the counted peak, and `VmPeak` on Linux, on stderr); tests as in Verification; an exit-2 test with a lowered cap; added to the dependency gate. About 600 lines. | task 3, task 5 |
| 8 | `cargo xtask limits`: both peaks per case in `limits.json`, the baseline `ops/metrics/limits-peaks.json` and the failure rules (§4), the `memory-cap` and #146 cases; specs/tooling/limits.md and nightly.md, and the gate's lift condition in specs/core/memory-budget.md and specs/tooling/cli-dependency-gate.md. Closes #146. About 900 lines. | task 7 |
| 9 | macOS measurement: `cargo xtask ci macos` runs the harness at `--mib 64` with the footprint from `/usr/bin/time -l`, plus #165's re-resolve case; spec updates. Closes #165. About 500 lines. | task 8 |
| 10 | Fuzz: every target installs `Capped` at `H`; the nightly replays each corpus with a `--sanitizer none` build under `ulimit -v 2097152` and reports aborts as crashes; specs/tooling/fuzz.md and nightly.md. About 500 lines. | task 7 |
| 11 | Remove the model: rewrite specs/core/memory-budget.md (the cap, the charging rule, the measurement, the requirements list), delete `MemoryBudget`, `alloc_bytes`' chunk model, `ImageInputBudget` and `charge_output` with their tests and docs (crates/jpdf-traits/src/image.rs), update P7's "Enforced by". Mostly deletions, about 400 added lines. | task 6, task 8 |

Each task issue carries `Designed in: #236` and `Depends on:` lines and is a sub-issue of #236
(ops/TICK.md §8 step 5).

## What reviewers check against
**P7. One memory cap at the allocator, one charging path, measured** (docs/PRINCIPLES.md), which
this ADR adds. Its paths: `crates/`, `tools/jpdf-cli/`, `fuzz/`, `apps/macos/JPDFHelper/`,
`xtask/src/limits.rs`, `xtask/src/limits/`, `specs/core/memory-budget.md`. Its reviewer lines,
in short: allocations in the gated code go through `jpdf-mem`; each input loop that allocates
has a checkpoint; a backend call runs inside a reservation of its per-call limits; a binary or
target that holds the engine installs `Capped`; no cap, soft-cap margin or baseline goes up
without a stated reason; and **the process rule:** a memory finding is a blocker or major only
with a concrete input and one of those violations, or a measurement (a harness or fuzz case that
fails). A predicted allocator behavior (fragmentation, arenas, retention, rounding) with no
measurement is a minor whose fix is a new harness case, never a model term (ADR 0010's "an
unproven bound"). A finding about code that doesn't exist yet is answered by the requirements
list (below; from task 11 the list in specs/core/memory-budget.md): `disputed: covered by ADR 0017
requirement <n>` when one covers it, else the PR adds it to the spec's list, and no issue is
filed.

## Requirements on future callers
1. **Every binary or target that holds the engine installs `Capped`** with `H` or lower and
   installs its gauge: the macOS engine helper (the Rust side of the XPC helper), the web worker
   (wasm32; `H` below the worker's memory maximum), every fuzz target, and any new tool that
   opens untrusted files.
2. **A front end that opens several documents at once** shares one process cap; it may divide
   the cap among documents with reservations, but never raises it.
3. **The first decode or raster phase on the jpdf-cli path** adds its hostile cases to the
   harness (for decode: #146's history with a maximal decode after it, a JBIG2 decode with page
   data and globals at their caps) and passes them on Linux and macOS before its PR changes the
   dependency gate's allowed set.
4. **Every backend adapter** (`jpdf-backends`) calls its crate only inside an `admit`
   reservation of the call's per-call limits (output plus working bytes), and its conformance
   suite checks the output limit at the exact boundary (at the cap passes, one past fails),
   reading the buffer's capacity, not its length (the #231 item 3 concern).
5. **Phase outputs that outlive their phase** (a decoded stream kept while it is rendered) are
   charged to the resolve cache's retained budget or dropped first: that budget is a cache policy
   that keeps re-parsing bounded, and it now charges requested capacities.
6. **Threads:** a new thread that allocates in the engine is created by code in the gated crates
   or `jpdf-cli`, and its stack size is set (#114); stacks are outside the counter and inside
   the measured margin.

## Consequences
- **Easier:** a memory review has a short checklist and a measurement instead of an argument; a
  new structure needs no term and no new default; macOS gets a real cap; the image path needs no
  allowance bookkeeping; several hundred lines of model and its tests go.
- **Harder:** library code can't use `Vec::push` or `format!` directly, so it is wordier; the
  error messages that today build a `String` must be static or go through the path. One crate
  holds hand-written `unsafe`, which needs the owner's amendment and careful review whenever it
  changes. Every allocation pays one atomic add (a counter update per alloc and dealloc).
- **Not covered:** an allocation history that no harness case or fuzz input reaches can still
  push the OS peak past the margin without passing the counter; the invariant then fails as a
  crash in the judge, and the fix is a new case and a lower `H` or the region design of §5.
  Memory the counter doesn't see (stacks, the binary, C allocations inside libc) is bounded only
  by the measurement. Until task 7 there is no counter at all, and until task 8 nothing measures:
  the decode and raster gate stays on until then.
