# 0013. Multi-step xtask tools hide which step ran and which failed

Status: proposed (design issue #357; queued by the Planner 2026-11-26, ops/TICK.md §9 step 2a).
A design ADR merges 24 hours after the latest of: its PR marked ready, its last `ai-review`
success, the owner's last `/release`, unless the owner comments `/hold` (ADR 0011 §7;
ops/TICK.md §8 step 5).

## The problem class
`cargo xtask nightly` (xtask/src/nightly.rs, xtask/src/nightly/extra.rs; spec
specs/tooling/nightly.md) is a sequence of side-effecting steps: load the previous results,
build `jpdf-cli`, check it, probe the memory limit, start the watchdog, the limits cases, the
fetch, the regress runs, `regress.json`, the diff, the stale `nightly.json` removal, the bench,
the fuzz pass, `nightly.json`. Its spec fixes their order (item 15), which failures stop the
sequence and which don't (items 10 and 11: "the regress part still runs" after a limits tool
failure), and how the per-step results merge into one exit code (item 10: 1 beats 75 beats 4
beats 0). The group, 6 open issues on that tool:

- **#350** (PR #349 review round 1, R1.4, minor): the limits part runs before the fetch and the
  regress part runs after a limits tool failure, but only the exit-code merge `with_limits` is
  tested. A refactor that moves the limits call after `nothing_to_run`'s early return, or
  returns early on a limits `Err`, passes every test.
- **#352** (PR #351 review round 2, R2.1, minor): `extra::run` returns one `bool`, so its test
  of a stale `nightly.json` that can't be removed passes through the later write failure alone;
  a mutation that ignores the failed removal still passes.
- **#302** (part 3 of #19): files issues from the night's results, which adds another step to
  the same sequence and reads the exit code to know whether `DIR` is this night's (item 10).
- **#19**: the nightly umbrella.
- **#322** (PR #321 R1.5) and **#323** (PR #321 R3.1): the stderr tail and the executable check.
  They are in the same files but are not instances of this class (see the table below).

The spec says it itself: its Security self-check lists "the order of steps inside `run`" under
"Not covered by tests". The same shape exists, smaller, in `cargo xtask ci linux`
(xtask/src/main.rs: four checks, "Run every check even if an earlier one failed", merged by
"first non-success").

**The class:** a tool whose exit code depends on several side-effecting steps computes that
code inside the code that performs the steps, folding each step's result into a shared value
(`failed: bool`, a `u8`) as it goes. A test sees only the folded value, so it can't pin which
steps ran, in what order, which one failed, or that a later step ran after an earlier failure.

## Why point fixes don't converge
Each part added its own fold, and each review found the next blind spot in the fold before it:

- Part 1 (PR #321) put the precedence in `exit_code(failed, fetch_code)` and a `failed` bool in
  `regress` (diff write, unstarted runs).
- #25 (PR #349) added the limits part with a second fold, `with_limits(regress, limits)`, and
  a call order inside `run` that no test reaches: R1.4, now #350.
- Part 2 (PR #351) added `extra::run`, a third fold into one `bool` across four steps (removal,
  bench, fuzz, write): R2.1, now #352.
- #302 would add a fourth.

#350 and #352 each propose their own fix (injected steps for one; an error list for the other).
Done one by one, they give two shapes for the same problem and leave the next step to invent a
third. The missing decision is one rule for how such a tool reports its steps: each step's
outcome as a value, the sequence decided in one place that tests can drive with fakes, and the
exit code as a function of the outcomes alone.

## The invariant
In a `cargo xtask` command whose spec fixes an order of side-effecting steps or says that a
step runs after another step failed:

1. each step's result is a value of one shared type, `steps::Outcome` (`Ok`, `Finding` for a
   measured failure such as a failed limits case, `Infra(reason)` for exit 75, `Failed(reason)`
   for a tool failure), recorded under the step's name; no step's result is merged into
   another's before the end;
2. which steps run, in what order, and what a failure stops is decided by one driver function
   that calls the steps through an injected interface (a trait or a struct of closures, like
   `extra::Hooks` today) and returns the log of the steps that ran, with their outcomes;
3. the exit code is a pure function of that log: `Failed` gives 1, else `Infra` 75, else
   `Finding` 4, else 0 (specs/tooling/nightly.md item 10's precedence, which `with_limits` and
   `exit_code` encode today in two places);
4. the real implementation of each step is a thin adapter from the existing function's result
   to an `Outcome`; it decides nothing about order.

Checkable from outside: drive the driver with a fake that fails step X; the log must list
exactly the steps the spec says run after X, in the spec's order, and the exit code must be the
table's.

## Enforcement
1. **Types: used.** `xtask/src/steps.rs` holds `Outcome`, the log type and `exit_code(&log)`;
   the driver's signature returns the log, and the step interface's methods return an
   `Outcome` (plus any data a later step needs, such as the fetched jobs). The type can't stop
   a step from ignoring an error inside itself, so levels 2–4 cover the rest.
2. **A lint or gate: not used.** A check that a tool "doesn't fold results early" would be a
   text scanner over Rust source; the owner had one such scanner removed for that reason (PR
   #151, owner instruction of 2026-11-22: "A text scanner over Rust source can always be
   bypassed"), and ops/TICK.md §4 says not to build enforcement tooling in answer to a finding.
3. **A measurement: not applicable.** The property is about which steps run, not about time,
   memory or output size.
4. **Reviewer vigilance: used**, for the adapters and the wiring: P4 in docs/PRINCIPLES.md.

## Verification
Per tool, one test drives the driver with a recording fake:
- all steps `Ok`: the log is the spec's full order;
- for each step, a run where it alone fails (`Failed`, and `Infra` or `Finding` where the step
  can return them): the log holds exactly the steps the spec runs after that failure, and the
  exit code is the table's;
- one table test of `exit_code` over every pair of outcome kinds, so the precedence is pinned
  once for every tool.

For #350 this pins "limits before the fetch" and "regress after a limits tool failure"; for
#352, a failed removal shows as its own `Failed` entry whatever the write does.

**What broken implementation still passes:** a real adapter that maps an error to `Ok` (the
fake replaces it; each adapter is one expression, checked by the reviewer, and the function it
wraps keeps its own unit tests); order inside one step (a step that does two things in the
wrong order: split it into two steps where its spec fixes that order); a `run` that calls a real
step outside the driver (the reviewer checks that `run` only builds the real interface and calls
the driver); the real build, fetch and CI wiring, which stay outside tests as today.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #350 | folded into task 1 (closed with a link once task 1 is filed) | its order tests are task 1's driver tests |
| #352 | folded into task 2 (closed with a link once task 2 is filed) | its test needs `extra`'s steps as separate outcomes |
| #302 | kept; its `Depends on:` gains task 1 | its issue-filing step is a step of the driver (Requirements on future callers) |
| #19 | kept | umbrella; closes after #302 |
| #322 | kept, no follow-up | stderr capture in `run_child`, inside one step; not this class |
| #323 | kept, no follow-up | the executable check is one step's own logic; not this class |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `xtask/src/steps.rs` (`Outcome`, the log, `exit_code` and its table test) and the nightly's `run`/`regress` sequence moved onto a driver over an injected step interface, replacing `with_limits` and `exit_code(failed, fetch_code)`; the order tests of #350 (limits before the fetch; regress after a limits tool failure; nothing after an early stop) and one run per failing step; specs/tooling/nightly.md items 10, 15 and "Tests" updated, and the "Not covered by tests" line about the order removed. About 600 added lines. | #357 |
| 2 | `extra::run` returns its steps' outcomes (stale-summary removal, bench, fuzz, `nightly.json` write) to the task 1 driver instead of one `bool`; #352's test (a failed removal is its own entry when the write succeeds); the spec's item 14 and "Tests" updated. About 250 added lines. | task 1 |

## What reviewers check against
P4 (added by this ADR), governing `xtask/src/`:

> **P4. A multi-step tool reports each step; one driver decides the order**
> - **Invariant:** a `cargo xtask` command whose spec fixes an order of side-effecting steps, or
>   runs a step after another failed, records each step's result as a `steps::Outcome` (`Ok`,
>   `Finding`, `Infra`, `Failed`) under the step's name; one driver function, calling the steps
>   through an injected interface, decides which run and in what order and returns the log; the
>   exit code is `steps::exit_code(&log)` (`Failed` 1, else `Infra` 75, else `Finding` 4, else 0).
>   A tool whose every step stops it on the first failure, in source order, needs no driver.
>   The nightly doesn't hold it yet (see Enforced by).
> - **Decided in:** ADR 0013.
> - **Enforced by:** not yet: ADR 0013's tasks (designed in #357) add the types in
>   `xtask/src/steps.rs` (the driver returns the log; steps return an `Outcome`) and each
>   driver's tests with a recording fake (the full order, one run per failing step, the
>   `exit_code` table); the real adapters and `run`'s wiring stay reviewer vigilance. Until
>   then the nightly folds its results in `with_limits`, `exit_code` and `extra::run`'s
>   `bool`; `cargo xtask ci linux` stays the known exception until a PR changes its checks
>   (ADR 0013, Requirements on future callers).
> - **Reviewers check:** a new or moved step is called by the driver, not by `run` or another
>   step; its real adapter maps every error of the function it wraps to `Failed` or `Infra` and
>   decides nothing about order; no step's result is folded into a `bool` or an exit code before
>   the end; the driver's tests cover the new step's failure.
> - **Paths:** `xtask/src/`.

## Requirements on future callers
- #302's issue filing is a step of the nightly driver with its own `Outcome` (a `gh` failure is
  `Failed` or `Infra` by the spec's rule), and its position in the order is in the driver and
  its tests.
- `cargo xtask ci linux` (xtask/src/main.rs) moves onto `steps.rs` in the next PR that adds,
  removes or reorders one of its checks; until then it is the known exception (four checks, no
  order the spec fixes).
- A new multi-step tool (a nightly-like command, `cargo xtask limits` if it ever continues
  after a failed precondition) is built on `steps.rs` from its first PR.
- A tool whose steps all stop the tool on their first failure, in source order, with one
  result (today `cargo xtask limits`: build, check, probe, watchdog, cases) needs no driver:
  `?` on each step is the order, and its spec states it. It needs one once a step runs after
  another's failure.

## Consequences
- Easier: a test pins order and continue-after-failure with fakes, and reviews of a new step
  check one driver instead of a fold spread over three functions; the exit-code precedence is
  written and tested once.
- Harder: each step needs a named adapter and a fake, which adds lines to small tools; data
  handed between steps (the jobs from the fetch to the regress runs and the bench) goes through
  the driver.
- Not covered: what happens inside a step (its own unit tests), the real wiring of `run` (the
  reviewer), and tools that stop on the first failure (no driver needed, above).
