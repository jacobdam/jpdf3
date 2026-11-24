# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **Planner 2026-11-24** (PR #307, merged fea1992): `ops/QUEUE.md` rewritten from the labels. Split #19 into #300–#302,
  #16 into #303–#304, #15 into #305–#306; readied #25 (on #300), #277, #294; moved #14 → M9, #20 → M1, #22 → M2;
  removed #21's dead `in-progress`.
- `cargo xtask next-session` decides steps 1–5, 7, 8, 10 and 11; step 6 is checked by hand until #275. Step 3 skips
  an own PR on the owner's `/hold` and an `owner/` branch PR (PR #299, 335d6ca, which also carried #296's commits).
- PR #296 closed unmerged and #295 closed (2026-11-24): its change is on main through PR #299 (335d6ca).
- PR #309 (#246, task 4 part 1: `ops/areas.toml`, `cargo xtask issue-lint`) is open.
- **PR #312** (#249, task 6 part 1/2: Design: product session, TICK §10, next-session step 8 rule k, label
  `design-product`) is **escalated to design** (`needs-design`, draft, `ai-review` failure): 4 rounds + final check;
  R4.1–R4.3 closed, the final check's only finding is R5.1 (minor, one missing assertion). Part 2/2, `docs/PRODUCT.md`
  v1, is #311 (`design-product`); the v1 trigger fires once #312 merges.
- The judge's M0 gate passes (nightly 45179218903, 2026-11-23: `gates.M0.pass`, `current_milestone` M1), but M0 is not
  tagged: its backlog isn't empty (rule j) and PLAN §8's pilot fire drill (#21, owner steps #208) is still open.
- PR #232 (#187 part 1) merged (48f63d9). Part 2: #231 (sev:high, M2, `deferred`).
- **#260** (tagged, `human`): work #242 then #245 first. Not an instruction until the owner posts an untagged comment
  on it (ADR 0011 §7).
- #236 (memory ADR, `deferred`) is the first design queue item. #146 stays open, `deferred` (waits for #25 or #236).
- #21 waits on #19 (#300–#302) and the owner's drills (#208); AC.4 goes into docs/retros/0000-pilot.md with them.
  Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Next step
0. `cargo xtask next-session` (TICK §2). Step 6 by hand first: PR #312 is `needs-design` → a Design: technical
   session (§8) on it (a ruling on R5.1, or a split).
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
