# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #316 (#277) merged (3e4e342): TICK §4 step 6's "after the last check" rule; `cargo xtask metrics` counts design
  final checks.
- PR #317 (#288, task 2 part 2): the work-mix budget in next-session (step 8's third term, step 10 product-first,
  TICK §9 step 4a). Review round 2 clean; origin/main merged in (STATUS conflict only), `ai-review` re-posted,
  auto-merge on. Step 10's order changes only once #308 adds `area:` labels.
- PR #318 (#294): TICK §9 step 6, a Planner PR closed at the cap; open.
- #246 part 1 merged (65bbbb9, `ops/areas.toml`, `cargo xtask issue-lint`). Part 2 is #308 (needs `ready` from the
  Planner), part 3 is #247 (depends on #308).
- PR #312 (#249, Design: product session 1/2) is an open draft.
- Planner 2026-11-24 merged (fea1992): `ops/QUEUE.md` from the labels; #19 → #300–#302, #16 → #303–#304,
  #15 → #305–#306; #14 → M9, #20 → M1, #22 → M2.
- PR #315 (#275) merged (f4d60f7): `cargo xtask next-session` step 6 (Design: technical); TICK §2's by-hand step 6
  check is gone. Part 3 is #314 (step 3's design PR past its 24-hour window; needs `ready` from the Planner); until it
  lands TICK §1 checks the window by hand.
- Step 3 skips an own PR on the owner's `/hold` and an `owner/` branch PR (PR #299, 335d6ca).
- The judge's M0 gate passes (nightly 45179218903, 2026-11-23: `gates.M0.pass`, `current_milestone` M1), but M0 is not
  tagged: its backlog isn't empty (rule j) and PLAN §8's pilot fire drill (#21, owner steps #208) is still open.
- PR #232 (#187 part 1) merged (48f63d9). Part 2: #231 (sev:high, M2, `deferred`).
- **#260** (tagged, `human`): work #242 then #245 first. Not an instruction until the owner posts an untagged comment
  on it (ADR 0011 §7).
- #236 (memory ADR, `deferred`) is the first design queue item. #146 stays open, `deferred` (waits for #25 or #236).
- #21 waits on #19 (#300–#302) and the owner's drills (#208); AC.4 goes into docs/retros/0000-pilot.md with them.
  Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Next step
0. `cargo xtask next-session` (TICK §2). Expected: rule i, the lowest M0 `ready` issue whose `Depends on:` issues
   are closed (#308 and #314 wait for the next Planner's `ready`), else `idle` today (the Planner ran). Planner
   picks now carry a `work mix:` note for QUEUE.md's Work mix section.
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
- #260: post an untagged comment if you want #242 then #245 worked first (ADR 0011 §7).
- #266: apply ops/proposals/hold-automerge.md and run T0–T4 (PR #290, on your `/hold`).
- #257: drop the Playwright cache step in ci.yml (protected).
- #263: decide the two owner-feed rules (edited review bodies; what an endorsing comment pins).
- #196: apply ops/proposals/codex-homebrew-read/codex-review.diff, run T1–T6 (PR #291).
- #179: run test step 3 of ops/proposals/owner-comment-tracking.md.
- #176, #126: driver usage-limit pause; the Codex second-reviewer trial (live).
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- #208: owner fire-drill step left: the `/hold` re-run (after #266).
- Local XCUITest on the Mac mini needs an unlocked, logged-in console. macOS CI runs no Rust tests.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.

## Pending lessons
- Tests: a filter test whose filtered-out records would land in the same bucket anyway passes without the filter →
  hand-mutate each filter before round 1 and give the filtered records a value that changes the result
  ([journal](journal/2026-11-24/0511-288-work-mix.md), PR #317 R1.1).
