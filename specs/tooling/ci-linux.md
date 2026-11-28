# `cargo xtask ci linux`: the order of its checks

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (it runs the journeys' web E2E tests; ops/judge/CONTRACT.md §2)
Status: implemented

## Behavior
`cargo xtask ci linux --out DIR` (ops/judge/CONTRACT.md §2) runs five checks, each one even
when an earlier one failed, so one run shows every problem. Since #369 the order is decided by
one driver, `ci_linux::drive` in `xtask/src/ci_linux.rs`, which records each check's result as
a `steps::Outcome` under its name; the exit code is `steps::exit_code` of that log
(docs/PRINCIPLES.md P4, ADR 0013). The order, and why it is fixed:

1. `web`: the web build and the Playwright run (`web::ci`). It removes an earlier `DIR/site/`
   and `DIR/e2e.json` first and writes `DIR/e2e.json`; it writes `DIR/site/` only after a
   successful build.
2. `dashboard`: copies the committed snapshot to `DIR/site/dashboard/data.json`
   (specs/tooling/dashboard.md, item 2). It comes after `web`: run before, its copy would be
   removed with the earlier `DIR/site/`, or find no `site/dashboard/` to copy into.
3. `check-specs`: specs/tooling/check-specs.md, after `web` has written `e2e.json`.
4. `fuzz`: the short fuzz run and the `fuzz/` format check (specs/tooling/fuzz.md).
5. `arlington`: the vendored model's pins, parse and compile check (specs/tooling/arlington.md).

Each check prints what it printed before #369 (`xtask: web: ok`, `xtask: dashboard: no page,
nothing copied`, `check-specs: ok`, `xtask: fuzz: no crash`, `xtask: arlington: …`, or its
error), and the log itself prints nothing.

## Persistence
None: the checks write only under `DIR` (and the fuzz tool's own work directories).

## Undo
Not applicable.

## Errors
Every check's failure is `Failed` (exit 1); no check of `ci linux` is an infrastructure failure
(75) or a finding (4) today. A failed check doesn't stop the later ones. The dashboard step's
own results (specs/tooling/dashboard.md, Errors): a missing snapshot or a missing
`site/dashboard/` is `Ok` with its note; a snapshot that isn't a regular file, is larger than
1 MiB or can't be copied is `Failed` with `dashboard: <reason>`.

## Limits
No new limit. Each check keeps its own (specs/tooling/dashboard.md: the 1 MiB snapshot copy,
boundary-tested in `xtask/src/ci_linux.rs`, where the copy lives, #433). The driver runs a fixed five steps, no loop.

## Security
Inputs are the repository's own files; nothing comes from the network beyond what each check
already fetched before #369. Self-check:
- Arithmetic: none in the driver.
- What broken implementation still passes? A driver that skips a step after a failure, or
  swaps two steps, fails the recording-fake tests (the full order with every step `Ok`, and
  with each step failing alone, five runs). A dashboard adapter that copies into `DIR` instead
  of `DIR/site/`, or maps an error to `Ok`, fails the adapter test. A `main` that bypasses the
  driver, or a real adapter for `web`, `check-specs`, `fuzz` or `arlington` that maps a failure
  to `Ok`, still passes: the real wiring is reviewer vigilance (P4, "Enforced by").
- Filters, gates, parsers, rules: the one rule is "every step runs, in this order"; it has the
  should-run cases above (each failing step alone, with every other step `Ok`). A run where
  nothing was checked can't happen: the driver has no condition.
- What the tests can't see: the real checks' own behavior (their own tests), and that `main`
  calls `drive` with the real adapter (the reviewer).

## Accessibility
Not applicable (a command-line tool).

## Platforms
Linux CI (`project-linux`) and local runs on macOS; the driver has no platform code.

## Performance
No change: the same checks run once each.

## Tests
`xtask/src/ci_linux.rs` unit tests: the order with every step `Ok` (exit 0); each step failing
alone, the other four `Ok`: all five still run in order, exit 1; the dashboard step's mapping
(copied, no page, no snapshot, an error); and the real dashboard adapter against a scratch
`DIR`, copying into `DIR/site/dashboard/data.json` and failing on a snapshot that is a
directory.
`xtask/tests/check_specs.rs` also runs the real `ci linux` in fixture trees and checks
exit 1; `ci_linux_runs_the_check_after_writing_e2e` pins the order of its last three checks'
lines (check-specs, fuzz, arlington), which covers part of the real wiring.
