# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #127 (owner #125): driver proposal + TICK §0 "Leave nothing running". Auto-merge goes on once ai-review is posted.
  If it is still open, check its CI first (rule c). The owner applies the driver diff (#128).

## Last session
- **#125** (rule a): a finished `claude -p` lingers while a ScheduleWakeup timer is pending (6 of 40 sessions, ~77 min
  idle), and the driver read any failure as a usage limit (the CLI's `rate_limit_event` lines are in every log).
  Proposal `ops/proposals/driver-session-exit.md` with a tested diff (37 scenarios after 3 review rounds),
  owner-todo #128. The second session (19:30 UTC) finished the reviews after the first lost its re-review to the
  CLI's 10 min background-task ceiling.

## Previous session
- **#81 1/2** (rule d, PR #123 merged): the image conformance suite; part 2/2 is #122.

## Next step (suggested order, TICK letters)
0. a: owner issue **#124** (the AI reviewer posts its review as a PR comment): TICK §4 wording, then reply on #124.
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
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
