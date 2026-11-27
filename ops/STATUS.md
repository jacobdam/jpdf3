# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- main is green: ci run 45669593183 for 6daac8e (PR #349's merge) passed.
- #301 (nightly 2/3) merged (PR #351); its R2.1 is #352 (no `ready`). Then #302 (part 3, files issues; depends on
  #322, PR #362: the nightly keeps the last 4 KiB of stderr) reads `nightly.json` and `limits.json` as well as `regress.json`.
- #305 (dashboard 1/2) merged (PR #354). The page is #353 (`ready`): finished work on branch
  `t/353-dashboard-page` (75d990f); merge origin/main into it and open the PR. #306 (judge metrics) now depends on #353.
- Planner 2026-11-26 (PR #358): `ready` on #350, #352, #353; filed design issue #357 (`ready`) for the 6 open
  `cargo xtask nightly` issues, so the next pick is a Design: technical session on it.
- Work mix 0 of 20: #353 (`apps/web/`) is the only open M0 issue that counts as milestone work; the mix rises once
  M0 closes and M1 opens (ops/QUEUE.md, Work mix).
- ADR 0012's one-time moves are done (`Ops` milestone; #308, #248 → M1, #250 → M3, #251 → M4; #247 deferred).
- The judge's M0 gate passes (nightly 45179218903, 2026-11-23), but M0 is not tagged: its backlog isn't empty (rule j)
  and PLAN §8's pilot fire drill (#21, owner steps #208) is still open.
- #187 part 2: #231 (sev:high, M2, `deferred`). #236 (memory ADR, `deferred`) is the first design queue item; #146
  stays `deferred`.
- #25 (PR #349, merged): `cargo xtask limits`, run by the nightly (exit 4 on a failed case). Its deferral #350 (order
  test) waits for the Planner.
- #21 waits on #19 (#302) and the owner's drills (#208); AC.4 goes into docs/retros/0000-pilot.md with them.
  Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Next step
0. `cargo xtask next-session` (TICK §2). #357's design PR #359 is open, so rule i comes next (#303, …
   lowest first; #323 is PR #364, auto-merge on).
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
- An exec oracle for permission modes with a shell-script fixture → give the owner read (0o500, not 0o100): the
  interpreter must read the script ([journal](journal/2026-11-27/0020-323-exec-check.md)).
