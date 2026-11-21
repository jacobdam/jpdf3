# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #135 (#106): decoder working memory and operations budget. Auto-merge goes on after ai-review; if it is
  still open, check its CI (rule c).

## Last session
- **#106** (rule d, trait contracts 2/3): `Limits` gains `max_decode_working_bytes` (256 MiB, now the decode term of
  the 2 GiB budget) and `max_decode_ops` (2^32); `Decoded` reports `usage`. The image suite probes both at exact
  boundaries, requires the same usage at every at-limit probe, and measures an `extra_work` claim as a difference
  between two equal-length inputs with the same output. 6 review rounds, all posted on the PR. Follow-ups: #136
  (heavy-memory claim), #137 (work caps on damaged-input probes), #109 (filter-chain intermediate not charged).

## Previous session
- **#83 1/2** (rule d, PR #133 merged): bench watchdog, npm --ignore-scripts, bounded spec reads.

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#107, #109, #113, #114, #116, #117, #122, #132, #134, #136, #137**.
   - **#99 soon after:** the text scan is advisory; the license property needs Rollup's module graph.
   - **Before #16 admits any adapter:** the suites (#116, #117, #122, #136, #137) and #107.
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105, #115.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next retro (rule h): by 2026-11-28.** Inputs from audit 0002 §6:
  - TICK puts d above a due audit, so the audit can starve, and this session deviated (an incident);
  - STATUS "in flight" is stale as soon as its PR merges;
  - the template's `Area:` list lacks `tooling`.
- **Next audit:** due at 10 merges since audit 0002 (9 once #135 merges) or by 2026-11-28 (see the carry-overs in docs/audits/0002.md).

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
