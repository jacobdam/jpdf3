# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #151 (#146, sev:high): on owner `/hold`. Draft, `ai-review` = success on f3a22d0. Do not mark it ready or
  enable auto-merge until the owner comments `/release`; then enable auto-merge.**
  - It carries out the owner's scope-cut instruction (comment on PR #151, 02:31 UTC). The tripwire is removed. The gate
    is a plain spec rule: no decode or raster on the jpdf-cli path until #146 closes.
  - Content: `heap_high_water` (the largest phase counted twice); six lowered defaults; peak estimate 1909.84 MiB.
    The subagent approved in rounds 8 and 15.
  - Follow-ups: #154 (enforce the gate structurally), #156 (sev:medium, M10: what closes #146; non-main arena
    sub-heaps). #146 stays open (the cross-phase build-up).
  - The owner also commented `/halt` on PR #151. It isn't a defined command (no `halt` label, no `ops/HALT`).
- **PR #157 (#155, owner issue): draft, #155 `blocked`, `ai-review` = failure on 6e799c2, waiting for the owner.**
  It applies review convergence (ADR 0010) to TICK §4. Its own final check found R5.1; the fix (fee5f2a, plus a
  one-line wording fix in 6e799c2) is unreviewed. Options are posted on #155 and PR #157. Act only on an owner comment
  on either: accept, run one more final check, or change.

## Last session
- #155 / PR #157 (rule a): ADR 0010 + TICK §4 rewrite (rubric, delta reviews, a 4-round cap per issue, a final check).
  4 rounds + the final check, then to the owner per the new step 6. Journal 2026-11-22 02:45.
- Before that: #146 / PR #151 (rule d), 15 rounds, now on owner `/hold`.

## Next step (suggested order, TICK letters)
0. PR #151: wait for the owner's `/release`. PR #157: wait for an owner comment on #155 or PR #157. Don't touch
   either before.
   Until #157 merges, TICK §4 on main is still the old loop, with no defined exit.
1. **g: AUDIT is overdue**: 18+ merges since audit 0002. d is now empty apart from #146.
   - Security issues: high is #146 (in flight). Medium/low wait in M10: #114, #116, #117, #141, #156 (medium); #83, #122,
     #132, #134, #136, #137 (low).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: handled by owner issue #155 / PR #157 (ADR 0010) once it merges; the retro checks its first numbers.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - the template's `Area:` list lacks `tooling`.

## Blockers / owner-todo
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live. It ran every round on PR #151 (status ok each time).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
- 8+ merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
