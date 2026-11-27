# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **#386 (owner instruction, `human`): replace ops/STATUS.md with `cargo xtask status` and per-session lesson files.**
  "picked up" posted; split into #390 (part 1, `cargo xtask status`, PR #393 merged, 0c36f04), #391 (part 2, pending
  lesson files in `ops/lessons/pending/`: PR #396, 2 rounds, `ai-review` success, auto-merge on) and #392 (part 3,
  delete STATUS.md and its rules; its PR carries the "done:" for the owner comment). Rule a keeps picking the owner
  comment until part 3 merges. Follow-ups #394 (R4.1 of #393) and #397 (R2.1 of #396), both `Ops`.
- #302 (nightly 3/3 part 1): PR #384 merged (1a95aa6). It added ops/proposals/nightly-file-issues.md: a session still
  has to file the owner issue for it with `owner-todo` (TICK §0). Part 2 (fuzz and bench findings) is #383, without
  `ready`, depending on #361.
- #348 (J-PC-08 fixture details): PR #388, owner `/approve` posted.
- Follow-ups without `ready`: #377, #371 (`Ops`); #369, #366, #367 (M0); #380 (M10).
- ADR 0013 (#357) merged. Task 1 #360 merged (PR #382). Task 2 #361 (`ready`) is next.
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
0. `cargo xtask next-session` (TICK §2). Expected: rule a (#386's owner comment): work #392 once PR #396 merges.
   A session still has to file the owner issue for ops/proposals/nightly-file-issues.md (above).
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

