# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #119 (the owner's Codex second-reviewer proposal): reviewed in 6 rounds, all findings fixed in the proposal,
  ai-review posted, auto-merge on. Once merged, the owner applies the protected diffs (step 1) and dry-runs
  Codex (step 2).
- This record PR (journal, STATUS, LESSONS).

## Last session
- **PR #119 review** (rule a, the owner's comment). 14 major findings fixed in ops/proposals/codex-second-reviewer.md
  (script confinement, sealed-corpus exposure through git discovery and symlinks, guard-rule fail-opens from
  regex blow-ups, TICK background-timeout). See the proposal's "Review record".

## Previous session
- Audit 0002 (rule g): 0 critical, 5 high (#112, #116, #117), filed #112–#117; see docs/audits/0002.md.

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#81, #82, #83, #106, #107, #109, #112, #113, #114, #116, #117**.
   - **#112 first, out of order:** CONSTITUTION 10 rests on the license check, and it is proven bypassable.
   - **Before #16 admits any adapter:** the suites (#81, #116, #117) and the decoder contracts (#106).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105, #115.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next retro (rule h): by 2026-11-28.** Inputs from audit 0002 §6:
  - TICK puts d above a due audit, so the audit can starve, and this session deviated (an incident);
  - STATUS "in flight" is stale as soon as its PR merges;
  - the template's `Area:` list lacks `tooling`.
- **Next audit:** after 10 more merges or by 2026-11-28 (see the carry-overs in docs/audits/0002.md).

## Blockers / owner-todo
- Local XCUITest on the Mac mini needs an unlocked, logged-in console. #52 (Automation Mode without a password) is
  done.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- Open proposal: ops/proposals/codex-second-reviewer.md (on main once #119 merges; the owner applies it). Suggested owner follow-up: a command
  size cap in guard.py and an explicit hook `timeout`; the current guard has quadratic inputs and a timed-out hook allows.
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
