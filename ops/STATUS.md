# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #178** (#173, owner: replies to owner instructions): auto-merge on, `ai-review` = success. When it has merged
  (§1 check): post `done: https://github.com/jacobdam/jpdf3/issues/173` on #173 (waiting on owner: apply #179), add
  `owner-todo` to #179 with `ready for owner: apply ops/proposals/owner-comment-tracking.md, test plan in §4`. If it
  hasn't merged, check its CI.
- Picked-up owner comments without a done reply (TICK §0, until #179 is applied): #173 (above).
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).

## Last session
- a: #173 → PR #178 (TICK §0 replies, AUTOMATION §12, proposal + 64-check harness), #179 filed for the owner part,
  backfilled done reply on PR #151. PR #177 merged before the session. Journal 2026-11-22 06:12.

## Next step (suggested order, TICK letters)
1. **d: #162** (sev:high, protected guard): write `ops/proposals/guard-tag-and-fail-closed.md` with the diff and a test
   plan; add `owner-todo` only when the proposal PR merges (TICK §0). #162's CX.4 is confirmed and small (drop the
   raw-text prefilter). Same pattern as #161:
   diffs and test scripts in `ops/proposals/<topic>/`, close the issue with the PR, and open an owner-todo issue to
   apply it once it merges. Then **#168** (sev:high, M2, engine work in budget.rs).
   - Skip **#174** under rule d: only the owner can act on it (apply and test the #161 proposal).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115, #154.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165; low #83, #122, #132, #134,
  #136, #137, #166.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: ADR 0010 (merged in #157, 2026-11-22); the retro checks its first numbers (PR #169: 2, PR #171: 3).
  - STATUS "in flight" goes stale as soon as its PR merges.
  - first numbers of the #173 reply rule (picked-up/done pairs, any instruction left without done).
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md §4 (the guard-trigger list) still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #179 (after PR #178 merges): apply the owner-comment tracking proposal.
- #174: apply the #161 proposal and run its owner test plan (T1–T4 and T10–T12; only the owner can run Codex outside
  the launcher). #162: proposal coming. The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live; it also ran the audit 0003 security review (status ok).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
