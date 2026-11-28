# 0017. The memory budget is predicted by a model, so every review finds another uncounted case

Status: proposed (design issue #236, an owner issue; queued by the owner in #238's first design
queue). A design ADR merges 24 hours after the latest of: its PR marked ready, its last
`ai-review` success, the owner's last `/release`, unless the owner comments `/hold` (ADR 0011 §7;
ops/TICK.md §8 step 5). The owner's `/approve` ends the window early. The owner asked for
`owner-todo` on the ADR PR itself ("so I review the design before it merges", #236), so the
label goes on the PR as well as on #236: `cargo xtask next-session` doesn't merge a design PR
that carries it, so this one merges only after the owner removes it (then the window or an
`/approve` applies as usual).

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
- A new crate `crates/jpdf-alloc` holds `CappedAlloc`, a `GlobalAlloc` that wraps `std::alloc::System`
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
  never raise it, as P1 does for `Limits`: a value above `H` is clamped to `H`.
- **The fuzz targets use `H` = 1024 MiB.** The judge's fuzz step runs AddressSanitizer builds
  with `-rss_limit_mb=2048`, and ASan's shadow memory, redzones and quarantine (256 MiB by
  default) come on top of the counted bytes, so a target capped at 1536 MiB could be reported
  as a libFuzzer out-of-memory crash on an input the cap handles.

### 2. One fallible charging path: `jpdf-mem`
- A new crate `crates/jpdf-mem` (no `unsafe`) is the only place in the engine's non-test code
  that allocates a buffer whose size is a runtime value. It offers fallible growth
  (`try_with_capacity`, `try_reserve`, `try_push`, `try_extend_from_slice`, `try_filled`,
  `try_box`, `try_arc`, a fallible B-tree and hash map insert, `try_collect`; the exact set is
  what the migrated code needs) and two checks for code it can't wrap:
  - `admit(bytes)`: room for a bounded call into code that allocates on its own (a third-party
    decoder in `jpdf-backends`, bounded by its per-call limits, P2), returning a `Reservation`
    that holds the bytes until dropped. The reservation isn't drawn down while the backend
    allocates, so during the call its memory is counted twice (reserved and allocated). That is
    accepted: it can only refuse a call early (a decode admitted near `S` that would have fit),
    never let one past `S`, and at the defaults a call's reservation (at most its output plus
    working bytes, 384 MiB for a decode) is far below `S`;
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
  2 GiB, and this ADR raises none of them (CONSTITUTION 4). That includes the two per-call
  bounds the model introduced: the shared allowance of one image decode's inputs
  (`Limits::image_input_budget`, `ImageInputBudget` with `charge_output`, #168, #187) and the
  resolve cache's charge per allocator chunk (`alloc_bytes`, at least 32 bytes per allocation).
  Deleting either would loosen a limit (an image's page data and globals could each reach the
  per-stream cap; the same retained budget would keep up to 32 times more small allocations), so
  both stay; only the whole-process arithmetic (`MemoryBudget`) goes.

### 3. Mechanical enforcement: `cargo xtask mem-lint`
A gate in `cargo xtask ci linux` runs clippy's `disallowed_methods` over the **non-test code**
of every crate under `crates/` and of `tools/jpdf-cli`, except `jpdf-mem` and `jpdf-alloc`:
`--lib --bins --no-deps` with the default features, so `#[cfg(test)]` code isn't compiled,
`jpdf-cli`'s binary target is linted (it has no library target, and `--lib` alone skips it), and
the dependencies `jpdf-mem` and `jpdf-alloc` aren't linted through the crates that use them. It
uses its own configuration (`CLIPPY_CONF_DIR`), at `-D clippy::disallowed_methods`.
`jpdf-traits`' conformance suites and toy backends (`pub mod conformance`, test support that the
backends' tests call and the engine never does) move behind a `conformance` cargo feature that
only dev-dependencies enable (task 5), so the gate's default-feature build doesn't compile
them; every other module of `jpdf-traits` is gated.

The list denies the std APIs that allocate a runtime-sized or growing buffer: the sized
constructors, reserve and resize methods, `vec![x; n]`, the growth methods of `Vec`,
`VecDeque`, `String`, the hash and B-tree maps and sets, slice and `str` copies and repeats
(`to_vec`, `repeat`, `concat`, `join`), `collect` into a collection, `Box::new`, `Arc::new`,
`Rc::new`, `format!`, `to_string`, and `Read::read_to_end` and `read_to_string` (the #141 path).
It works on resolved paths, not text, so a renamed import or a re-export doesn't bypass it
(PR #151: the owner rejected text scanners). Each entry has a fixture that the gate must reject.
It doesn't touch the workspace's clippy run or the protected root `clippy.toml`, and no code may
`allow` it (CONSTITUTION 5).

**What it can't list:** allocating methods of traits that are mostly used without allocating.
`disallowed_methods` resolves `v.clone()` to `Clone::clone`, so a clone of input data (a
`Vec<u8>`, an `Object`) can be denied only by denying every clone; the same holds for
`ToOwned::to_owned`, `From::from` and `Into::into` into a `Vec` or `String`, and
`Extend::extend`. These stay allowed. A copy of input-sized data through them is a P7 reviewer
check, the checkpoint in the surrounding loop bounds how far it can go past `S`, and the
measurement catches what is left.

### 4. Verification by measurement
- `cargo xtask limits` (#25) records, for each case, the counted peak (`JPDF_MEM_REPORT=1` makes
  `jpdf-cli` print it to stderr at exit) and the OS peak (`VmPeak` from `/proc/self/status`,
  printed by `jpdf-cli` on Linux; on macOS, the `peak memory footprint` that `/usr/bin/time -l`
  reports). On Linux the runs stay under `ulimit -v 2097152`.
- **The baseline** `ops/metrics/limits-peaks.json` holds, per platform (`linux`, `macos`), per
  `--mib` value that a job runs, and per case, the counted peak and (macOS) the footprint. A
  case fails (exit 4, as a limits finding does) when:
  - its counted peak passes its baseline by more than 10% plus 1 MiB (exactly baseline × 1.1 +
    1 MiB passes, one byte more fails);
  - on Linux, its OS peak passes 2 GiB − 64 MiB (exactly that passes, one byte more fails);
  - on macOS, its footprint passes 2 GiB − 64 MiB or its footprint baseline by more than 25%
    plus 16 MiB (the same boundary rule): the counter sees only live bytes, and memory libmalloc
    keeps after a free (#165) shows only in the footprint;
  - its counted peak is 0 (no allocator installed).

  The counted peak of single-threaded code is the same on every run, so its tolerance is tight;
  the OS numbers are noisy, so the Linux OS peak gets only the ceiling and the macOS footprint a
  wide tolerance. Raising a baseline is like weakening an assertion: the PR says why
  (CONSTITUTION 14).
- **Where it runs, and what that misses.** Each PR: `cargo xtask ci linux` (the `project-linux`
  job, which the judge's verdict needs) runs the harness at `--mib 64` under the Linux limit,
  so a PR that worsens a case's counted peak fails before it merges. Nightly: `cargo xtask
  nightly` runs it at the default 500 MiB and replays the fuzz corpora (below). macOS: `cargo
  xtask ci macos` (the `project-macos` job) runs it at `--mib 64`; that job doesn't run on pull
  requests, so a macOS-only regression is found on the next push to main, not before the merge.
  Fuzzing itself runs on Linux only.
- **Cases** added to today's seven: `memory-cap` (a valid file under a lowered
  `JPDF_MEM_CAP_MIB`, expecting exit 2 `limit exceeded: memory`); #146's history (many cached
  growing containers, then requests of growing sizes; the parse-only part today, the decode part
  with the first decode phase); `many-small` (the largest number of tiny allocations the limits
  allow, such as maximal dictionaries of one-byte names kept in the cache: the C allocator's
  per-allocation overhead, which the counter doesn't see, grows with the count, so this case
  checks the margin where it is thinnest); and on macOS #165's re-resolve probe (one large
  uncached object resolved many times).
- The fuzz targets install `CappedAlloc` at their `H` (1024 MiB, §1), so an allocation past the
  cap is a refused charge or a saved crash. The nightly also replays each target's corpus with a
  `--sanitizer none` build under `ulimit -v 2097152`: AddressSanitizer reserves terabytes of
  shadow address space at start, so a sanitizer build can't run under a real `RLIMIT_AS`.
- specs/core/memory-budget.md is rewritten (task 11) as the cap, the charging rule, the
  measurement, the per-call bounds that stay (§2) and the living list of requirements on future
  code. `MemoryBudget` and its tests are deleted: it existed to make the sum fit.

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
   lower), counted by `jpdf_alloc::CappedAlloc`;
2. in the non-test code of the engine crates and `jpdf-cli` (the gate's scope, §3), every allocation whose size is a
   runtime value goes through `jpdf-mem`, which refuses it with a typed error once the counted
   bytes plus reservations plus the request would pass the soft cap `S = H − 128 MiB`; code
   that allocates on its own (a third-party decoder) runs only inside an `admit` reservation of
   its per-call limits;
3. on every case of the limits harness and every replayed fuzz input, the run ends with exit 0
   or 2, never an abort, its counted peak is within its baseline, on Linux its OS peak stays
   under 2 GiB − 64 MiB, and on macOS its footprint stays under 2 GiB − 64 MiB and within its
   baseline (§4).

A reviewer can say "this diff breaks it" for: an allocation in the gated code that isn't through
`jpdf-mem`, an input loop with no checkpoint, a backend call outside a reservation, a binary that
holds the engine without installing `CappedAlloc`, a raised cap or baseline, or a harness case whose
peak got worse.

## Enforcement
1. **Types:** `jpdf-mem`'s helpers return `Result`, so a refused charge can't be ignored
   silently; `Reservation` releases on drop; the gauge is installed once. `CappedAlloc` refuses past
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
  installed is caught by the harness's "counted peak is 0" rule. `JPDF_MEM_CAP_MIB` gets its own
  boundary tests in task 7: a value of exactly `H` is taken as `H`, `H` + 1 MiB (and any larger
  or malformed value) is clamped to `H` or refused as a usage error, and a lowered value makes a
  valid file fail with exit 2 `limit exceeded: memory`.
- `jpdf-mem`'s tests, with a fake gauge: a charge that reaches `S` exactly passes and one byte
  more is refused; a reservation counts until dropped; sums saturate. Still passing: a helper
  that checks the cap but then allocates infallibly. The lint doesn't cover `jpdf-mem` itself, so
  its tests must reach each helper's refusal through the gauge and its `try_reserve` failure
  through a request of `isize::MAX` bytes.
- The lint's fixtures: one per denied method, including through `vec!` and `format!`. Still
  passing: a std method that allocates but isn't on the list, and the trait methods §3 can't list
  (`Clone::clone`, `ToOwned::to_owned`, `From`/`Into` into a `Vec` or `String`,
  `Extend::extend`) applied to input-sized data. The list is checked against std's `alloc` API
  when the toolchain is updated (audit sessions), the trait methods are a P7 reviewer check, and
  the measurement catches what both miss.
- The harness: a `jpdf-cli` without the allocator fails (counted peak 0); one that leaks or
  grows fails its baseline; one that aborts at the cap is a `crash`; one that ignores
  `JPDF_MEM_CAP_MIB` fails `memory-cap`. Task 8 tests each failure rule of §4 at its exact
  boundary with fake runs: a counted peak of exactly baseline × 1.1 + 1 MiB passes and one byte
  more fails, an OS peak (and a macOS footprint) of exactly 2 GiB − 64 MiB passes and one byte
  more fails, a footprint of exactly its baseline × 1.25 + 16 MiB passes and one byte more fails,
  and a counted peak of 0 fails while 1 byte passes; a baseline looked up under the wrong
  platform or `--mib` key fails a test too. Still passing: a hostile allocation history that no
  case reproduces. The cases come from findings (§ the process rule) and the fuzzers explore the
  rest; this is the gap that stays open, and it is a missing test case, not a missing term.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #109 | kept closed; its model (`MemoryBudget`) is deleted by task 11 | the lowered defaults stay (CONSTITUTION 4); the whole-process arithmetic goes |
| #146 | folded into task 8, whose PR closes it; `deferred` until then | the cross-phase history becomes a harness case under the real limit, measured instead of bounded by a formula; the gate stays until then |
| #154 | kept closed; the gate stays | it keeps decode and raster off the jpdf-cli path until task 8 is merged and the first decode phase's PR passes its own harness cases (Requirements on future callers); task 8 restates the lift condition in specs/tooling/cli-dependency-gate.md |
| #156 | superseded (closed with a link to this ADR) | what closes #146 is decided here (task 8's cases under the cap); arena reservations are part of the OS peak the harness measures |
| #165 | folded into task 9, whose PR closes it | the macOS measurement runs #165's re-resolve probe as a case and fails it when the footprint passes its ceiling or baseline (§4), which is where memory libmalloc keeps after a free shows; the counter caps the live bytes |
| #168 | kept closed; `ImageInputBudget` stays as a per-call bound | removing the shared allowance would let page data and globals each reach the per-stream cap, a loosened limit (§2); under the cap, a decode that doesn't fit under `S` also fails with `LimitExceeded("memory")` |
| #187 | kept closed; `charge_output` stays with `ImageInputBudget` | it is how that per-call bound charges capacity |
| #231 | superseded (closed with a link to this ADR) | it is about code that doesn't exist yet, so its items become requirements on future callers: items 1 and 2 (the image caller charges each stage with `charge_output`, chunked growth, no speculative reservation at the cap) are requirement 7, item 3 (each adapter's output capacity at the exact cap) is requirement 4 |
| #141 | kept (sev:medium, M10) | the cap turns its 3 × growth into a refused charge or a cap abort, but the read itself should stop at the size + 1 byte |
| #114 | kept (sev:medium, M10) | the clamp is P1's; thread stacks are outside the counter and inside the measured margin |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `crates/jpdf-mem`: the helpers, `admit`/`Reservation`, `checkpoint`, the `Gauge` trait with a one-time install, the soft cap and `MemError`; tests with a fake gauge at the exact soft-cap boundary, reservation release and saturation; spec `specs/core/memory-charging.md`. About 700 lines. | #236 |
| 2 | `ops/proposals/allocator-exception.md`: CONSTITUTION 3's second sole exception (`crates/jpdf-alloc`, its limits as in §1) and the matching ops/judge/policy.py change, with a test plan. About 300 lines. | #236 |
| 3 | Owner: apply task 2's proposal (signed commits). Filed without `ready`; `owner-todo` once task 2's PR merges (ops/TICK.md §0). | task 2 |
| 4 | Migrate jpdf-syntax onto `jpdf-mem` (lexer, parser containers, xref table, spans, trailer, resolve cache), checkpoints in its input loops, `LimitExceeded("memory")`; boundary tests with a fake gauge (a parse that crosses `S` fails with the error, never panics); `jpdf-mem` added to the jpdf-cli dependency gate. About 800 lines. | task 1 |
| 5 | Migrate jpdf-cos, jpdf-engine, jpdf-cli (`read_capped` through `jpdf-mem`) and the runtime modules of jpdf-traits (`limits.rs`, `raster.rs` and the rest outside `conformance`) the same way, with their boundary tests; move `jpdf-traits`' `pub mod conformance` and its toy backends behind a `conformance` cargo feature that only dev-dependencies enable (§3). About 800 lines. | task 4 |
| 6 | `cargo xtask mem-lint` (§3) in `cargo xtask ci linux`: `--lib --bins --no-deps` with the default features, the configuration, one fixture per denied method (a library and a binary fixture, so a skipped target fails), the crate list, spec `specs/tooling/mem-lint.md`. About 600 lines. | task 5 |
| 7 | `crates/jpdf-alloc`: `CappedAlloc` (count, peak, refusal at `H`), its `Gauge` impl; installed in jpdf-cli with `JPDF_MEM_CAP_MIB` (lower only, its boundary tests as in Verification) and `JPDF_MEM_REPORT` (the counted peak, and `VmPeak` on Linux, on stderr); tests as in Verification; an exit-2 test with a lowered cap; added to the dependency gate. About 600 lines. | task 3, task 5 |
| 8 | `cargo xtask limits`: both peaks per case in `limits.json`, the baseline `ops/metrics/limits-peaks.json` keyed by platform and `--mib`, the failure rules of §4 with their exact-boundary tests (Verification), the `memory-cap`, #146 and `many-small` cases, and a `--mib 64` run in `cargo xtask ci linux`; specs/tooling/limits.md, ci-linux.md and nightly.md, and the gate's lift condition in specs/core/memory-budget.md and specs/tooling/cli-dependency-gate.md. Closes #146. About 1000 lines. | task 7 |
| 9 | macOS measurement: `cargo xtask ci macos` runs the harness at `--mib 64` with the footprint from `/usr/bin/time -l` and the macOS failure rules of §4 (ceiling and footprint baseline), plus #165's re-resolve case; spec updates. Closes #165. About 500 lines. | task 8 |
| 10 | Fuzz: every target installs `CappedAlloc` at its `H` of 1024 MiB (§1); the nightly replays each corpus with a `--sanitizer none` build under `ulimit -v 2097152` and reports aborts as crashes; specs/tooling/fuzz.md and nightly.md. About 500 lines. | task 7 |
| 11 | Remove the model: rewrite specs/core/memory-budget.md (the cap, the charging rule, the measurement, the per-call bounds that stay, the requirements list), delete `MemoryBudget` (crates/jpdf-syntax/src/budget.rs) and its tests, update P7's "Enforced by". `ImageInputBudget`, `charge_output` and `alloc_bytes` stay (§2). Mostly deletions, about 400 added lines. | task 6, task 8 |

Each task issue carries `Designed in: #236` and `Depends on:` lines and is a sub-issue of #236
(ops/TICK.md §8 step 5).

## What reviewers check against
This ADR adds one entry to docs/PRINCIPLES.md, quoted here as merged; reviewers of later PRs on its paths get it in their brief (ops/TICK.md §4).

> ## P7. One memory cap at the allocator, one charging path, measured
> - **Invariant:** in a process that holds the engine, the bytes Rust's global allocator has
>   handed out never pass the hard cap `H` (1536 MiB, or lower; 1024 MiB in the fuzz targets),
>   counted by `jpdf_alloc::CappedAlloc`. In the non-test code of the crates under `crates/` and
>   of `tools/jpdf-cli` (the `cargo xtask mem-lint` scope, ADR 0017 §3), every allocation whose
>   size is a runtime value goes through `jpdf-mem`, which refuses it with a typed error
>   (`LimitExceeded("memory")`, CLI exit 2) once the counted bytes plus reservations plus the
>   request would pass the soft cap `S = H − 128 MiB`; code that allocates on its own (a
>   third-party decoder) runs only inside an `admit` reservation of its per-call limits. Every
>   limits-harness case and replayed fuzz input ends with exit 0 or 2, its counted peak within
>   its baseline (`ops/metrics/limits-peaks.json`, per platform and `--mib`), on Linux its OS
>   peak under 2 GiB − 64 MiB, and on macOS its footprint under 2 GiB − 64 MiB and within its
>   baseline. The safety of the budget is never argued from a model of allocator behavior.
> - **Decided in:** ADR 0017.
> - **Enforced by:** nothing yet; ADR 0017's tasks add it. Then: `jpdf-mem`'s fallible API
>   (types, task 1); `cargo xtask mem-lint` in `cargo xtask ci linux` (task 6); `CappedAlloc`'s
>   refusal at `H` (task 7, after the owner applies the CONSTITUTION 3 exception, tasks 2 and 3);
>   the limits harness in `cargo xtask ci linux` at `--mib 64` on every PR, in the nightly at
>   500 MiB, and on macOS in `cargo xtask ci macos` (pushes to main only), and the nightly fuzz
>   replay (tasks 8 to 10). Checkpoints in input loops, reservations around backend calls, copies
>   through trait methods the lint can't list, and the allocator's installation in each binary
>   stay reviewer vigilance. Until task 8 is merged, decode and raster stay off the jpdf-cli path
>   (specs/core/memory-budget.md, Gate; specs/tooling/cli-dependency-gate.md).
> - **Reviewers check:** an allocation in the gated code goes through `jpdf-mem` (from task 6 the
>   lint sees the std methods it lists; a method it misses is still a finding); input-sized data
>   isn't copied through `clone`, `to_owned`, `From`/`Into` or `extend` outside `jpdf-mem`; each
>   loop over input that allocates calls `checkpoint()`; a backend call runs inside a reservation
>   of its output plus working bytes; a binary or fuzz target that holds the engine installs
>   `CappedAlloc` and its gauge; no cap, soft-cap margin, ceiling or baseline goes up without the
>   PR saying why (CONSTITUTION 14). **The process rule:** a memory finding about allocator
>   behavior is a blocker or major only with a concrete input and one of those violations, or a
>   measurement (a harness or fuzz case that fails). A predicted allocator behavior
>   (fragmentation, arenas, retention, rounding) with no measurement is a minor whose fix is a new
>   harness case, never a new model term or default. ADR 0010's other grades still apply as they
>   are: a false statement, or a missing test for a behavior or limit the PR claims (an
>   exact-boundary test of `S`, say), is a major. A finding about code that doesn't exist yet is
>   answered by ADR 0017's Requirements on future callers or by the "Requirements on future code"
>   list in specs/core/memory-budget.md (which exists today and which ADR 0017 task 11 keeps):
>   disputed as covered by one of them, or added to that spec list in the same PR; no issue is
>   filed for it.
> - **Paths:** `crates/`, `tools/jpdf-cli/`, `fuzz/`, `bindings/`, `apps/macos/JPDFHelper/`,
>   `xtask/src/limits.rs`, `xtask/src/limits/`, `specs/core/memory-budget.md`.

## Requirements on future callers
1. **Every binary or target that holds the engine installs `CappedAlloc`** with `H` or lower and
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
   that keeps re-parsing bounded, charged per allocator chunk as today (`alloc_bytes`).
6. **Threads:** a new thread that allocates in the engine is created by code in the gated crates
   or `jpdf-cli`, and its stack size is set (#114); stacks are outside the counter and inside
   the measured margin.
7. **The image-decode caller** (jpdf-filters, M2; from #231) takes `Limits::image_input_budget()`
   once per image decode and charges every stage output with `ImageInputBudget::charge_output`
   (its capacity), never `charge` with a length; each stage on the image path grows its output in
   chunks and never reserves the cap speculatively; a test through the real caller shows a stage
   output with spare capacity charged by capacity (the steps in specs/core/memory-budget.md,
   "The inputs of one image decode share one 128 MiB allowance"). Its buffers also go through
   `jpdf-mem`, so the process cap applies on top.

## Consequences
- **Easier:** a memory review has a short checklist and a measurement instead of an argument; a
  new structure needs no term and no new default; macOS gets a real cap; about 500 lines of model and its tests
  (`MemoryBudget`) go.
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
