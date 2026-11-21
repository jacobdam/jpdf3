# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #123 (#81 part 1/2, image conformance suite): auto-merge on once ai-review is posted. If it is still open,
  check its CI first (rule c). Part 2/2 is #122.

## Last session
- **#81 1/2** (rule d): the image suite caps fixture tolerance (2 from 8 bits, 0 lossless) with a mean bound,
  refuses flat fixtures (per colour component), requires `Complete` at the limits, a salvage cut for JPEG, JPX
  and CCITT, ColorTransform fixtures that override the implied transform both ways, JBIG2 globals, CCITT K < 0,
  0, ≥ 2 and alignment, and CCITT parameter limits before any data (u32 overflow included). 3 review rounds.

## Previous session
- **#112** (rule d, PR #121 merged): web license-check bypasses closed; #99 is still the durable fix.

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
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
