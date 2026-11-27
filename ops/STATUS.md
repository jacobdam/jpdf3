# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- main: ci run 45918554776 for c105253 passed; run 45920627986 (PR #368's merge, 4cf0fdb) was in progress at 01:06 UTC.
- #301 (nightly 2/3) merged (PR #351); its R2.1, #352, is folded into #361. Then #302 (part 3, files issues; depends on
  #360) reads `nightly.json` and `limits.json` as well as `regress.json`.
- #353 (dashboard page) merged (PR #368). Follow-up #369 (R1.3, R1.4), M0 without `ready`.
- #306 (dashboard 2/2, judge metrics) merged (PR #370). Follow-up #371 (R2.2, `Ops`, without `ready`). M0's
  milestone-work issues are all closed.
- #323 (nightly exec check): PR #364, round 1 clean after fixes, `ai-review` success, auto-merge on (merged main twice
  more for STATUS conflicts).
- #344 (driver test's 200-PR bound): PR #365 merged. Follow-ups #366 (the test needs GNU `touch -d`, fails on
  macOS) and #367 (R1.1, a `grep -x` without `-F` mutant), both M0 without `ready`.
- ADR 0013 (#357, PR #359) is in its 24-hour window (`owner-todo` on #357). Its tasks #360, #361 are `ready` (Planner
  2026-11-27) and wait on #357; they fold #350 and #352 (closed). #302 also depends on #360. #322 merged (PR #362).
- Work mix 0 of 20: after #306 no open M0 issue counts as milestone work; the mix rises once M0 closes and M1
  opens (ops/QUEUE.md, Work mix).
- ADR 0012's one-time moves are done (`Ops` milestone; #308, #248 → M1, #250 → M3, #251 → M4; #247 deferred).
- The judge's M0 gate passes (nightly 45179218903, 2026-11-23), but M0 is not tagged: its backlog isn't empty (rule j)
  and PLAN §8's pilot fire drill (#21, owner steps #208) is still open.
- #187 part 2: #231 (sev:high, M2, `deferred`). #236 (memory ADR, `deferred`) is the first design queue item; #146
  stays `deferred`.
- #21 waits on #19 (#302) and the owner's drills (#208); AC.4 goes into docs/retros/0000-pilot.md with them.
  Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Next step
0. `cargo xtask next-session` (TICK §2). Expected: `worker design-window #359` once its window passes,
   then rule i (#303, #323, lowest first).
1. **Next audit:** due at 10 product merges since audit 0006 or 2026-11-30 (`cargo xtask next-session` counts).
2. **Next retro (0003): by 2026-11-30.** Measure retro 0002's two action items (0 conflicts in ops/journal/**,
   conflict-only sessions ≤ 1 in 20; PRs at 4 rounds ≤ 10%, no "passes when nothing ran" audit finding), retro 0001 item 1's
   audit lag under the scheduler, the product-merge share (baseline 23%) and the records-only share
   (baseline 24%, #243's measure). Carried inputs: 5 applied proposals
   still say "proposed" (audit 0006); audit 0004 AC.3/AC.4; Codex timeouts and brief-only reviews (PR #268, audits
   0005–0006).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217, #223, #252,
  #263; low #83, #122, #132, #134, #136, #137, #166, #193, #203, #218, #262.

## Blockers / owner-todo
- #257: drop the Playwright cache step in ci.yml (protected).
- #263: decide the two owner-feed rules (edited review bodies; what an endorsing comment pins).
- #196: applied on main (5fdb119, T1–T2 pass); the machine-specific Homebrew tests on the Mac mini are left.
- #179: test step 3 of ops/proposals/owner-comment-tracking.md: the agent replied to both test comments on PR #318;
  your check of the feed is left.
- #176, #126: driver usage-limit pause; the Codex second-reviewer trial (live).
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- #208: owner fire-drill step left: the `/hold` re-run (#266's T1–T4 on PR #324 passed; check whether that covers it).
- Local XCUITest on the Mac mini needs an unlocked, logged-in console. macOS CI runs no Rust tests.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.

## Pending lessons
- A merge of origin/main that stops on conflicts while main changed a protected path → the guard blocks every Bash
  call (even `git add`/`git commit`); `git merge --abort`, `git merge origin/main -X theirs`, then re-apply your side
  in a new commit ([journal](journal/2026-11-27/0048-353-dashboard-page.md)).
- Merging origin/main locally is blocked by the guard when the merge brings a protected-path change from main →
  set the conflicting record file to main's copy in a record commit, push, then `gh pr update-branch N`
  (ops/journal/2026-11-27/2300-347-design-window.md).
