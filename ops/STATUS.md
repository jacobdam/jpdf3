# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #151 (#146, sev:high): on owner `/hold`. Draft, `ai-review` = success on f3a22d0. Do not mark it ready or
  enable auto-merge until the owner comments `/release`; then enable auto-merge.**
  - Follow-ups: #154 (enforce the gate structurally), #156 (sev:medium, M10). #146 stays open.
  - The owner's `/halt` on PR #151 isn't a defined command (no `halt` label, no `ops/HALT`).
- PR #171 (#161, proposal: Codex launcher isolation): `ai-review` = success on 777beb9, auto-merge on. If it hasn't
  merged, check its CI. Merging closes #161; the owner's apply and test steps are in #174.

## Last session
- d: #161 → PR #171: ops/proposals/codex-review-isolation.md and ops/proposals/codex-review-isolation/ (diffs for
  protected ops/driver/codex-review.sh and driver.sh, tests: 45 + 18 checks). 3 review rounds (R1: 6 majors; R2, R3:
  approve). New #174 (owner-todo, sev:high): owner applies and runs the test plan. Journal 2026-11-22 04:10.

## Next step (suggested order, TICK letters)
0. PR #151: wait for `/release`. Don't touch it before.
1. **d: #162** (sev:high, protected guard): write `ops/proposals/guard-tag-and-fail-closed.md` with the diff and a test
   plan; `owner-todo` stays. #162's CX.4 is confirmed and small (drop the raw-text prefilter). Same pattern as #161:
   diffs and test scripts in `ops/proposals/<topic>/`, close the issue with the PR, and open an owner-todo issue to
   apply it. Then **#168** (sev:high, M2, engine work in budget.rs).
   - Skip **#174** under rule d: only the owner can act on it (apply and test the #161 proposal).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115, #154.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165; low #83, #122, #132, #134,
  #136, #137, #166.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: ADR 0010 (merged in #157, 2026-11-22); the retro checks its first numbers (PR #169: 2, PR #171: 3).
  - STATUS "in flight" goes stale as soon as its PR merges.
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md §4 (the guard-trigger list) still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #174: apply the #161 proposal and run its owner test plan (T1–T4 and T10–T12; only the owner can run Codex outside
  the launcher). #162: proposal coming. The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live; it also ran the audit 0003 security review (status ok).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
