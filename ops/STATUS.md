# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #104 (#77): engine memory margins from audit 0001 (auto-merge on). It includes journal, STATUS and LESSONS.
  #77 closes when it merges.

## Last session
- **#77 security** (rule d). PR #103 (#75 flaky) merged during the session; its own session had finished it.
  - CLI `info`: `stat` before open. Non-regular files and oversized files are refused unopened.
  - The open never blocks: `O_NONBLOCK` (plus `O_NOCTTY` on Linux), arch-gated values. `fstat` re-checks the
    opened file; the read is capped at `max + 1`.
  - Lexer: reals are parsed in place (no copy) and `i64::MIN` parses. xref: `/Prev` is matched without cloning.
  - `clippy::arithmetic_side_effects` is denied in jpdf-syntax, jpdf-cos, jpdf-engine and jpdf-cli. Workspace-wide
    is #105.
  - Review: 6 minors and 4 nits, all fixed; the delta review found nits only.

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#79, #80, #81, #82, #83**. Tighten the suites (#80, #81) and the
   contracts (#79) **before** #16 admits any adapter.
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next audit (rule g):** at the 10th merge after e82b6a6 (8 so far; #104 makes 9), or by 2026-11-28.
  - Audit housekeeping includes the Arlington pin bump through `jpdf-arlington vendor`.
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md.

## Blockers / owner-todo
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- No open proposals in ops/proposals/.
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
