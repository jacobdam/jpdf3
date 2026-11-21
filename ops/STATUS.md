# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #129 (owner #124): TICK §4 now posts every AI review round and its triage on the PR; guard proposal
  ops/proposals/guard-inert-post-text.md. If it is still open, check its CI first (rule c).

## Last session
- **#124** (rule a): TICK §4 posts each reviewer round verbatim as a PR comment, then a triage comment, and links
  the ai-review status to it; §6 posts audit reviews too. Guard proposal ops/proposals/guard-inert-post-text.md
  (owner-todo #130) lets review text pass verbatim without new bypasses (scratch copy: self-test 135/135, 27/27
  mutants caught). 5 review rounds.

## Previous session
- **#125** (rule a, PR #127 merged): driver proposal (owner-todo #128) and TICK §0 "Leave nothing running".

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#82, #83, #106, #107, #109, #113, #114, #116, #117, #122**.
   - **#99 soon after:** the text scan is advisory; the license property needs Rollup's module graph.
   - **Before #16 admits any adapter:** the suites (#116, #117, #122) and the decoder contracts (#106).
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
- Open proposal: ops/proposals/codex-second-reviewer.md (merged; the owner applies it). Suggested owner follow-up: a command
  size cap in guard.py and an explicit hook `timeout`; the current guard has quadratic inputs and a timed-out hook allows.
- Open proposal: ops/proposals/guard-inert-post-text.md, owner-todo #130. Until it is applied, review comments reword
  guard-trigger words. Also in #130: the guard cmux and keyword rules are text matches, which any agent can dodge
  (`c''mux`). A real barrier (cmux socket auth, a permission deny rule) is an owner decision.
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
