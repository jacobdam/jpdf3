# 0014. Tests of protected files stop running once their proposal is applied

Status: proposed (design issue #420; queued by the Planner 2026-11-28, ops/TICK.md §9 step 2a).
A design ADR merges 24 hours after the latest of: its PR marked ready, its last `ai-review`
success, the owner's last `/release`, unless the owner comments `/hold` (ADR 0011 §7;
ops/TICK.md §8 step 5).

## The problem class
Agents can't change protected paths (CONSTITUTION 12), so a change to `ops/driver/`,
`.github/workflows/` and the like is written as `ops/proposals/<topic>.md` with a test script
next to it, and the owner applies it with signed commits. Seven proposals carry tests, and all
seven are applied:

| Proposal | Test | Applied in | Target on main |
|---|---|---|---|
| driver-keep-open-worktrees | `test.sh` | eaaa553 (PR #355) | `ops/driver/driver.sh` |
| driver-portable-df | `test.sh` | 917d8bc (PR #339) | `ops/driver/driver.sh` |
| driver-session-exit | `test.sh`, `fakebin/` | 538a3e4 (PR #144) | `ops/driver/driver.sh` |
| codex-review-isolation | `test-codex.sh`, `test-driver.sh` | f790f37 (PR #181) | `ops/driver/codex-review.sh`, `ops/driver/driver.sh` |
| codex-homebrew-read | `test-policy.sh` | 5fdb119 (PR #291) | `ops/driver/codex-review.sh` |
| hold-automerge | `test.sh` | bb8c0f5 (PR #290) | `.github/workflows/owner-commands.yml` |
| owner-comment-tracking | `test-owner-issues.sh` | 66f4d3c (PR #184) | `ops/driver/owner-issues.sh` |

The group (#420):
- **#342** (closed): the keep-open-worktrees proposal. Its PR #343's R3.1 became **#344**
  (closed by PR #365: the 200-PR read bound).
- **#366**: that test calls GNU-only `touch -d`, so it fails on macOS, where the driver runs.
- **#367**: the same test misses a `grep -x` without `-F` mutant (PR #365 R1.1).
- **#195**: `codex-homebrew-read/test-policy.sh` misses later `-c permissions` arguments and
  indirect writes (PR #194 R3.1, R3.2; audit 0005 CX.4). The Planner held it as not workable:
  nobody had decided whether the test outlives its applied proposal.
- **#179** (closed): the owner part of owner-comment-tracking; its `test-owner-issues.sh` is the
  test of an applied proposal too (PR #421 R1.3).

Run on 2026-11-28 on macOS against the files on main (4420f6d), each test given the live
target as its argument:

| Test | Result |
|---|---|
| driver-keep-open-worktrees/test.sh | **8 cases fail**: `touch -d` is rejected, so no worktree is old (#366) |
| driver-portable-df/test.sh | pass |
| driver-session-exit/test.sh | pass (0 failures, about 3 min) |
| codex-review-isolation/test-codex.sh | **aborts in case 3**: `HB_READ[@]: unbound variable` |
| codex-review-isolation/test-driver.sh | pass (18) |
| codex-homebrew-read/test-policy.sh | pass (3) |
| hold-automerge/test.sh | pass |
| owner-comment-tracking/test-owner-issues.sh | pass (102) |

`test-codex.sh` broke without anyone noticing: its case 3 pulls the `FS_POLICY` block out of
`codex-review.sh` by its exact text, and the later codex-homebrew-read proposal (PR #291)
rewrote that block to use `HB_READ`. That proposal was tested with its own script only.

**The class:** a test of a protected file lives with the proposal that introduced it, not with
the file. Nothing lists it, nothing runs it, nothing says which file it tests (the proposal's
copy or the applied file) or on which platform it must pass. So once the proposal is applied,
the test is neither maintained nor retired: a later proposal for the same file doesn't run it,
a platform bug in it stays hidden, and each review finds one gap in one test.

## Why point fixes don't converge
- #344 strengthened the keep-open-worktrees test (a 200-PR boundary) while the test couldn't
  pass on the only platform the driver runs on (#366); PR #365's review saw only the cases.
- #367 and #195 ask for more cases in tests that no session or CI job runs again after the fix,
  so the next change to their target can break them the way #291 broke `test-codex.sh`.
- #195 stayed unworkable because the question "does this test still matter?" has no owner.

Each fix improves one script. The missing decision is where tests of protected files live,
what they run against, on which platforms, and who runs them.

## The invariant
1. Every test of a protected file is an entry in one manifest,
   `ops/protected-tests/manifest.toml`, next to the scripts it lists, under
   `ops/protected-tests/`. An entry names its script, the protected paths it tests (its
   targets, each matching a pattern of `ops/judge/protected.txt` and existing on the tree), the
   platforms it must pass on, the tools it needs and its helper files (fake programs and
   fixtures such as `fakebin/`).
2. An entry runs against its targets at their protected paths on the checked-out tree: the
   runner passes each target path to the script as its argument. To test a copy (a proposal's
   version), the runner passes the copy's path instead; it never writes, moves or replaces a
   file at a protected path (CONSTITUTION 12), so the protected file's bytes are the same after
   any run.
3. An entry's platforms include every platform where a target runs: macOS for `ops/driver/`
   (the Mac mini, AUTOMATION.md), Linux for a `.github/workflows/` file whose job runs on
   `ubuntu-*`. An entry may list more. A platform means every machine that runs the entry for
   it: macOS is both the Mac mini (sessions) and GitHub's `macos-15` runner (`ci macos`), Linux
   is the `ubuntu-*` runner (`ci linux`); an entry finds its tools on each of them (no
   hard-coded `/opt/homebrew/bin/python3.14`) or fails there.
4. Every entry passes on main on each of its platforms. A tool it needs that is missing fails
   the entry; it never skips it.
5. Once a proposal is applied, its `ops/proposals/<topic>/` directory holds no test and no copy
   of a protected file; its `.md` (with any `.diff`) stays as the record, with a Status line
   naming the commit that applied it and the manifest entries that test it.

Checkable from outside: list `ops/protected-tests/`, the manifest and the applied proposals'
directories, and run the runner on each platform.

## Enforcement
1. **Types: not applicable.** The tests are shell scripts over shell scripts and workflow
   files.
2. **A lint or gate: used.** `cargo xtask protected-tests` (task 2) will read the manifest, run
   every entry for the host platform against the live targets, and fail on: a failing entry; a
   missing tool; a file under `ops/protected-tests/` that no entry lists as its script or a
   helper; a target that doesn't exist or matches no pattern of `ops/judge/protected.txt` (so an
   entry can't point at a proposal's copy); a target that matches no row of the runner's
   platform table (a table maps target prefixes to their platforms), and an entry whose
   platforms break invariant 3.
   `cargo xtask ci linux` will run the Linux entries on every PR (task 3) and
   `cargo xtask ci macos` the macOS entries on main and nightly (project-macos skips PRs;
   task 4). Both are `cargo xtask`
   code, not protected, so no proposal is needed. Invariant 5 has no gate: "applied" is the
   owner's merge, which the runner can't tell from the tree.
3. **A measurement: used, in that the gate is one.** It will run each test against the file
   that is really deployed, so drift like `test-codex.sh`'s shows as a red run.
4. **Reviewer vigilance: used** for what the gate can't see: P5 in docs/PRINCIPLES.md (a test
   that ignores its target argument, invariant 5, a proposal's own test run).

**Lane.** A PR that changes `ops/protected-tests/` is in the strict lane, as one that changes
`ops/proposals/` is today (ADR 0012 §1): these tests pin the owner feed and the driver, so
dropping a case from one is a strict-list change. Moving the tests out of `ops/proposals/`
would otherwise take them off the strict list (`ops/` makes the new directory `process`), so
task 2 adds `ops/protected-tests/` to the strict list in ops/TICK.md §4 and ADR 0012 §1 in the
same PR that creates the directory.

**Proposals from now on.** A proposal that changes a protected file runs the runner with its
copy's path in place of the target's before round 1 and states the result in its PR, so a change that
breaks another proposal's test is seen then. A test it adds or changes for the new behavior
lives in its `ops/proposals/<topic>/` directory until the owner applies it, because the same
test in `ops/protected-tests/` would fail against today's file and turn CI red; the
proposal's apply steps move it into `ops/protected-tests/` and its manifest in the same signed
commit that changes the protected file. The runner will be able to run such a test (by its path
in the proposal's directory) in place of the manifest's script, without changing
`ops/protected-tests/` (task 2).

## Verification
- The runner's own tests (task 2): a fixture manifest with an entry that passes, one that
  fails, one whose tool is missing (fails, not skipped), a file no entry lists as script or
  helper (fails), a target that doesn't exist, one outside `ops/judge/protected.txt` (a
  proposal's copy) and one no platform row matches (each fails), a driver target listed only
  for Linux (fails), and a copy substitution that is seen by the entry (a copy that fails the
  fixture test turns the run red) while the target file's bytes are unchanged after the run,
  each with its near miss.
- The first real run (task 2's PR): every moved test passes against main's files on every
  platform it lists (macOS on the Mac mini; Linux in the PR's `ci linux` run or a local Linux
  container), with #366 and task 1 fixed first; the PR lists each entry's result per platform.
- CI (tasks 3 and 4): the entries' step shows in `ci linux`'s log on a PR and in `ci macos`'s on
  the hosted runner (task 4 runs it there with `gh workflow run ci.yml --ref <branch>` before
  merging); a hand-broken entry turns the step red in each task's PR description (shown, then
  reverted).

**What broken implementation still passes:**
- a script that ignores its target argument and tests something else (the reviewer reads the
  default target line, P5);
- a test whose cases pass vacuously (it extracts nothing, or every case checks a constant);
  each script's own self-checks (`test-codex.sh` and `test-policy.sh` already refuse a missing
  marker) and its review cover this, not the runner;
- a PR that breaks a macOS entry: PR CI runs only Linux entries, so it merges and main's
  project-macos turns red, which rule b picks up. A session's local checks run the runner on
  the Mac mini (task 5), which narrows that gap;
- an applied proposal whose directory still holds a test (invariant 5): reviewers and the next
  audit's housekeeping (ops/TICK.md §6) see it.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #342 | closed already; no change | applied in PR #355; its test moves in task 2 |
| #344 | closed already; no change | done in PR #365; its cases move with the test |
| #366 | kept; task 2 depends on it | the moved test must pass on macOS (invariant 3); the fix is the issue's own |
| #367 | kept | one more case in a test that is kept; it doesn't depend on the move (a `git mv` keeps the edit, whichever merges first) |
| #195 | kept, workable now | the test outlives its proposal and runs against `ops/driver/codex-review.sh` (invariant 2), so R3.1 and R3.2 are cases of a live test; the Planner can gate it into `ready` |
| #179 | closed already; no change | its `test-owner-issues.sh` moves in task 2 |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `ops/proposals/codex-review-isolation/test-codex.sh` against the live `ops/driver/codex-review.sh`: case 3 aborts (`HB_READ[@]: unbound variable`) since PR #291 changed the `FS_POLICY` block, and with the extraction fixed it would still fail, because it asserts `/opt/homebrew` is granted and the policy has 8 keys, which PR #291 changed on purpose (9 `HB_READ` directories, no `/opt/homebrew`). Make every case pass on macOS with the live file as its argument; each changed assertion, and any case removed because it only repeats `codex-homebrew-read/test-policy.sh`, is explained in the PR (CONSTITUTION 14). | #420 |
| 2 | `cargo xtask protected-tests` (specs/tooling/protected-tests.md): the manifest, the runner (on `xtask/src/steps.rs`, one step per entry, P4), the copy substitution for targets and tests, and the gate checks of Enforcement level 2. Move the eight tests of the table above (and `fakebin/`, as helpers) into `ops/protected-tests/` with the live targets as their defaults; tests that hard-code a host path (the Homebrew Python) find their tools instead. Add `ops/protected-tests/` to the strict list in ops/TICK.md §4 and ADR 0012 §1 (Lane, above). The PR lists each entry's result on every platform it lists. | #420, #366, task 1 |
| 3 | CI, Linux: `cargo xtask ci linux` moves onto `xtask/src/steps.rs` (ADR 0013, Requirements on future callers: the next PR that adds a check) with a protected-tests step for the Linux entries. | task 2 |
| 4 | CI, macOS: `cargo xtask ci macos` moves onto `xtask/src/steps.rs` too (`macos::ci` already runs its checks after an earlier failure, so a third check meets P4's trigger) with a protected-tests step for the macOS entries after the app tests; checked on the hosted runner before merging. | task 2 |
| 5 | Process text: `ops/proposals/README.md` gets the proposal life cycle (the runner with the copy before round 1; new tests in the proposal's directory; apply steps that move them); ops/TICK.md §3's local checks run `cargo xtask protected-tests` for a PR that changes `ops/protected-tests/` or `ops/proposals/`; the applied proposals' Status lines name their applying commit and their entries; their whole-file copies (`driver-keep-open-worktrees/driver.sh`, `driver-portable-df/driver.sh`, `owner-comment-tracking/owner-issues.sh`, `hold-automerge/owner-commands.yml`) are deleted. | task 2 |

Each task issue carries `Designed in: #420` and `Depends on:` lines (ops/TICK.md §8 step 3).

## What reviewers check against
P5 in docs/PRINCIPLES.md (added by this ADR):

> **P5. A protected file's tests run against the file, on its platform, from one manifest**
> - **Invariant:** every test of a protected file (CONSTITUTION 12) is an entry in
>   `ops/protected-tests/manifest.toml` with its script, targets, platforms, tools and helper
>   files; each target matches `ops/judge/protected.txt` and exists. The runner passes each
>   target's protected path to the script, or a copy's path when told to test a copy, and never
>   writes a file at a protected path. An entry's platforms include every platform where a
>   target runs (macOS for `ops/driver/`: the Mac mini and the hosted `macos-15` runner; Linux
>   for a workflow job on `ubuntu-*`); it passes on main on each, and a missing tool fails it. A
>   PR that changes `ops/protected-tests/` is in the strict lane. An applied proposal's
>   `ops/proposals/<topic>/` directory holds no test and no copy of a protected file.
> - **Decided in:** ADR 0014.
> - **Enforced by:** reviewer vigilance until ADR 0014's tasks land: today the tests still sit in
>   the applied proposals' directories and nothing runs them. Then `cargo xtask protected-tests`
>   (task 2: a failing entry, a missing tool, an unlisted file, a missing or unprotected target,
>   a target or platform the platform table rejects) in `cargo xtask ci linux` for the Linux
>   entries (task 3) and `cargo xtask ci macos` for the macOS ones (task 4). "Applied" has no
>   gate.
> - **Reviewers check:** until ADR 0014 task 2 has merged, only that a changed test of a
>   protected file still takes its target as an argument and passes on the target's platform.
>   From then on: a new or changed test of a protected file has a manifest entry with the
>   target's platforms and helpers, and takes the protected path from the runner; the runner
>   never writes a protected path. From ADR 0014 task 5 on (the proposal life cycle): a proposal
>   PR ran the runner with its copy's path and says which entries fail on purpose; a test it
>   changes stays in its own directory and its apply steps move it into `ops/protected-tests/`
>   in the same signed commit; an applied proposal's directory keeps only its `.md` and `.diff`
>   records.
> - **Paths:** `ops/protected-tests/`, `ops/proposals/`, `xtask/src/protected_tests*`.

## Requirements on future callers
- A new proposal for a protected file runs `cargo xtask protected-tests` with its copy's path in
  place of the target's before round 1, and its PR says which entries pass and which fail on purpose
  (a case that pins the behavior the proposal changes), with the replacement test in its
  directory.
- A proposal's apply steps (the owner's signed commit) move its new or changed tests into
  `ops/protected-tests/` and the manifest in the same commit as the protected change, so main is
  never red between the two.
- A test written for a protected file that no proposal touches (a missing case found in an
  audit) goes straight into `ops/protected-tests/` with a manifest entry: it tests the file on
  main.
- The guard's self-test (`.claude/hooks/guard.py --selftest`) and the judge's (`ops/judge/selftest.py`)
  live inside protected paths and are the owner's; this ADR doesn't move them.
- The runner is a multi-step tool: it runs every entry after an earlier one failed, so it is
  built on `xtask/src/steps.rs` from its first PR (P4, ADR 0013).

## Consequences
- Easier: a proposal sees at once whether it breaks an earlier proposal's test; #195-style
  findings have a live test to land in; the Planner can tell a kept test from a dead one.
- Harder: an apply commit has one more step (moving the tests), and the owner does it. A
  macOS-only failure reaches main before CI sees it, and turns the project-macos job red, so a
  driver test failure shows up next to the app's results until it is fixed.
- Not covered: whether the tests are strong enough (mutants such as #367's are found by review
  and audits, as before), and the protected files that have no test at all (the guard and
  the judge have their own self-tests, which stay where they are).
