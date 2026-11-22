# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #142 (#109): memory budget. Auto-merge is on after 6 review rounds (ai-review success on 92f3706). If it is
  still open, check its CI (rule c).

## Last session
- **#109** (rule d): `jpdf_syntax::MemoryBudget` + specs/core/memory-budget.md. Peak model = persistent terms +
  largest phase + itemized 256 MiB reserve; the defaults fit 2 GiB with 66 MiB spare after lowering
  `max_objects_per_parse` (2^20), `max_xref_entries` (2^22), `max_raster_working_bytes` (512 MiB) and
  `max_canvas_pixels` (2^23). The resolve cache now charges glibc chunk sizes; the stored trailer keeps ≤ 2 entries.
  New follow-ups: #141 (CLI read buffer), #146 (heap reuse across phases).

## Previous session
- **#113** (rule d, PR #140 merged): exact string room per token, bounded reference look-ahead.

## Next step (suggested order, TICK letters)
0. **AUDIT is overdue (rule g): 10+ merges since audit 0002.** TICK puts d above g, so a long security backlog
   starves the audit; two sessions in a row picked d. The next retro must reorder (see below). Until then, a session
   that finds main green and no own PR failing should weigh running the audit.
1. d (security) issues, lowest number first: **#114, #116, #117, #122, #132, #134, #136, #137, #141, #146**.
   - **#146 must be resolved before any decode or raster runs on the jpdf-cli path.**
   - **#99 soon after:** the text scan is advisory; the license property needs Rollup's module graph.
   - **Before #16 admits any adapter:** the suites (#116, #117, #122, #136, #137).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105, #115.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64; #83 after #132.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - TICK puts d above a due audit, so the audit starves (audit 0002 §6; this session and the last both picked d);
  - STATUS "in flight" is stale as soon as its PR merges;
  - the template's `Area:` list lacks `tooling`;
  - review rounds: PR #121 took 4 and PR #142 took 6, both past rule c's 3-attempt cap without blocking the issue
    (recorded as deviations). Decide whether the cap covers review rounds, and consider a pre-review self-check for
    "budget/model" PRs (list every allocation on the path).
- **Next audit:** overdue (see 0). Carry-overs in docs/audits/0002.md.

## Blockers / owner-todo
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is on main (PR #147); the owner dry run remains. #128 (driver
  session exit, PR #144) and #130 (guard inert post text, PR #145) are closed and applied. This session's review
  posts still reworded apostrophes and the keyword for unchecked code; check whether the applied guard still needs it.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`; the guard
  has quadratic inputs, and a timed-out hook allows the command.
- 7+ merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
