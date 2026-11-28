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
   targets), the platforms it must pass on and the tools it needs.
2. An entry runs against its targets at their protected paths on the checked-out tree. It runs
   against a copy only when the runner is told to put that copy in place of the target.
3. An entry's platforms include every platform where a target runs: macOS for `ops/driver/`
   (the Mac mini, AUTOMATION.md), Linux for a `.github/workflows/` file whose job runs on
   `ubuntu-*`. An entry may list more.
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
   missing tool; a script under `ops/protected-tests/` that no entry lists; an entry whose
   platforms break invariant 3 (a table in the runner maps target prefixes to their platforms).
   `cargo xtask ci linux` will run the Linux entries on every PR and `cargo xtask ci macos` the
   macOS entries on main and nightly (project-macos skips PRs), task 3. Both are `cargo xtask`
   code, not protected, so no proposal is needed. Invariant 5 has no gate: "applied" is the
   owner's merge, which the runner can't tell from the tree.
3. **A measurement: used, in that the gate is one.** It will run each test against the file
   that is really deployed, so drift like `test-codex.sh`'s shows as a red run.
4. **Reviewer vigilance: used** for what the gate can't see: P5 in docs/PRINCIPLES.md (a test
   that ignores its target argument, invariant 5, a proposal's own test run).

**Proposals from now on.** A proposal that changes a protected file runs the runner with its
copy in place of the target before round 1 and states the result in its PR, so a change that
breaks another proposal's test is seen then. A test it adds or changes for the new behavior
lives in its `ops/proposals/<topic>/` directory until the owner applies it, because the same
test in `ops/protected-tests/` would fail against today's file and turn CI red; the
proposal's apply steps move it into `ops/protected-tests/` and its manifest in the same signed
commit that changes the protected file. The runner will be able to run such a test in place of the
manifest's copy, without changing `ops/protected-tests/` (task 2).

## Verification
- The runner's own tests (task 2): a fixture manifest with an entry that passes, one that
  fails, one whose tool is missing (fails, not skipped), a script no entry lists (fails), a
  driver target listed only for Linux (fails), and a copy substitution that is seen by the
  entry (a target copy that fails the fixture test turns the run red), each with its near
  miss.
- The first real run (task 2's PR): every moved test passes on macOS against main's files, with
  #366 and task 1 fixed first; the PR lists each entry's result.
- CI (task 3): the Linux entries' step shows in `ci linux`'s log on a PR; a hand-broken Linux
  entry turns the step red in the task's PR description (shown, then reverted).

**What broken implementation still passes:**
- a script that ignores its target argument and tests something else (the reviewer reads the
  default target line, P5);
- a test whose cases pass vacuously (it extracts nothing, or every case checks a constant);
  each script's own self-checks (`test-codex.sh` and `test-policy.sh` already refuse a missing
  marker) and its review cover this, not the runner;
- a PR that breaks a macOS entry: PR CI runs only Linux entries, so it merges and main's
  project-macos turns red, which rule b picks up. A session's local checks run the runner on
  the Mac mini (task 4), which narrows that gap;
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
| 1 | `ops/proposals/codex-review-isolation/test-codex.sh` against the live `ops/driver/codex-review.sh`: case 3 aborts (`HB_READ[@]: unbound variable`) since PR #291 changed the `FS_POLICY` block. Make every case pass on macOS with the live file as its argument; a case that only repeats `codex-homebrew-read/test-policy.sh` may be removed, with the reason in the PR (CONSTITUTION 14). | #420 |
| 2 | `cargo xtask protected-tests` (specs/tooling/protected-tests.md): the manifest, the runner (on `xtask/src/steps.rs`, one step per entry, P4), the copy substitution for targets and tests, and the gate checks of Enforcement level 2. Move the eight tests of the table above (and `fakebin/`) into `ops/protected-tests/` with the live targets as their defaults, and give `ops/protected-tests/` an owner in `ops/areas.toml` (`process`). The PR lists each entry's result on macOS. | #420, #366, task 1 |
| 3 | CI: `cargo xtask ci linux` moves onto `xtask/src/steps.rs` (ADR 0013, Requirements on future callers: the next PR that adds a check) with a protected-tests step for the Linux entries; `cargo xtask ci macos` runs the macOS entries as a step of its own after the app tests. | task 2 |
| 4 | Process text: `ops/proposals/README.md` gets the proposal life cycle (the runner with the copy before round 1; new tests in the proposal's directory; apply steps that move them); ops/TICK.md §3's local checks run `cargo xtask protected-tests` for a PR that changes `ops/protected-tests/` or `ops/proposals/`; the applied proposals' Status lines name their applying commit and their entries; their whole-file copies (`driver-keep-open-worktrees/driver.sh`, `driver-portable-df/driver.sh`, `owner-comment-tracking/owner-issues.sh`, `hold-automerge/owner-commands.yml`) are deleted. | task 2 |

Each task issue carries `Designed in: #420` and `Depends on:` lines (ops/TICK.md §8 step 3).

## What reviewers check against
P5 in docs/PRINCIPLES.md (added by this ADR):

> **P5. A protected file's tests run against the file, on its platform, from one manifest**
> - **Invariant:** every test of a protected file is an entry in
>   `ops/protected-tests/manifest.toml` with its targets, platforms and tools; it runs against
>   its targets at their protected paths unless the runner is told to put a copy in their place;
>   its platforms include every platform where a target runs (macOS for `ops/driver/`, Linux for
>   a workflow job on `ubuntu-*`); it passes on main there, and a missing tool fails it. An
>   applied proposal's `ops/proposals/<topic>/` directory holds no test and no copy of a
>   protected file.

Paths: `ops/protected-tests/`, `ops/proposals/`, the runner's future module
(`xtask/src/protected_tests*`).

## Requirements on future callers
- A new proposal for a protected file runs `cargo xtask protected-tests` with its copy in place
  of the target before round 1, and its PR says which entries pass and which fail on purpose
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
