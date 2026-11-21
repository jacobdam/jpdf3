# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #121 (#112, web license-check bypasses): reviewed in 4 rounds, auto-merge on. #112 closes when it merges.
- PR #119 (the owner's Codex proposal) merged; the owner applies the protected diffs.

## Last session
- **#112** (rule d): the web license check now refuses unscanned `src/` extensions, `import.meta` (not env),
  package paths in any text, scheme imports (`data:`, `file:`), undecodable specifier characters, non-literal
  `import(`, `@jsx` pragmas; it guards csp.ts by shape, tsconfig, PostCSS configs up the tree and 31 config keys,
  and pins the CC-BY allowance to the registry tarball. The reviewer found 4 blockers, then 2, then 1 more;
  all fixed. #99 (the Rollup module graph) is still the durable fix.

## Previous session
- PR #119 review (rule a): 14 major findings fixed in ops/proposals/codex-second-reviewer.md.

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#81, #82, #83, #106, #107, #109, #113, #114, #116, #117**.
   - **#99 soon after:** the text scan is advisory; the license property needs Rollup's module graph.
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
- Open proposal: ops/proposals/codex-second-reviewer.md (merged; the owner applies it). Suggested owner follow-up: a command
  size cap in guard.py and an explicit hook `timeout`; the current guard has quadratic inputs and a timed-out hook allows.
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
