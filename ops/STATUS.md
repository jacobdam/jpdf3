# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- The audit 0002 PR (#111): docs/audits/0002.md, two doc reconciliations (PLAN §2.2, jpdf-cli spec), journal, STATUS,
  LESSONS. #111 closes when it merges.

## Last session
- **Audit 0002** (rule g). Scope: 3b6101b..fa63ae1 (13 merges, PR #71 through #110).
  - Four reviewers found 0 critical, 5 high, 9 medium and 31 low findings. The five highs:
    - two web license-checker bypasses that let tool-tree code reach the bundle (#112);
    - three conformance-suite holes (#116: Huffman codes > 9 bits, CBC resetting every 4 blocks; #117: hostile font
      glyph that is only junk).
  - Also: the lexer exceeds the string limit by 1 byte per name or odd hex token (#113); the clamp holds only at
    `Document::open` (#114); cleanup in #115; comments on #81, #82, #87 and #105.
  - Mutants on the changed core: 96% of viable mutants caught (177 mutants).

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#81, #82, #83, #106, #107, #109, #112, #113, #114, #116, #117**.
   - **#112 first, out of order:** CONSTITUTION 10 rests on the license check, and it is proven bypassable.
   - **Before #16 admits any adapter:** the suites (#81, #116, #117) and the decoder contracts (#106).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105, #115.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next retro (rule h): by 2026-11-28.** Inputs from audit 0002 §6:
  - TICK puts d above a due audit, so the audit can starve;
  - STATUS "in flight" is stale as soon as its PR merges;
  - the template's `Area:` list lacks `tooling`.
- **Next audit:** after 10 more merges or by 2026-11-28 (see the carry-overs in docs/audits/0002.md).

## Blockers / owner-todo
- Local XCUITest on the Mac mini needs an unlocked, logged-in console. #52 (Automation Mode without a password) is
  done.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- No open proposals in ops/proposals/.
- 7 merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
