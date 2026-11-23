# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #285** (#243, ADR 0011 task 1b 2/2): step 11 (`planner` or `idle`) replaces `legacy`; `Session:` lines;
  records go into the work PR as a record commit (TICK §4, §5); `Pending lessons` below. `ai-review` after 2 rounds,
  auto-merge on. From its merge on, every session follows the new TICK §5.
- **PR #276** (#245, ADR 0011 task 3 part 1): parked at the review cap (4 rounds + final check), `owner-todo`,
  `blocked`. Part 2: #275. Follow-ups: #277.
- `cargo xtask next-session` decides steps 1–5, 7 and 10 (PRs #269, #272, #274, #284) and step 11 (PR #285). Task 2
  (#244, the Planner session, `ops/QUEUE.md`, work-mix budget) is next in ADR 0011's order.
- **PR #232** (#187 part 1): parked for the owner (draft, `ai-review` failure on 0831d70 after 4 rounds + final
  check). No review round until an owner comment. Part 2: #231 (sev:high, M2, `deferred`).
- **#260** (tagged, `human`): work #242 then #245 first. Not an instruction until the owner posts an untagged comment
  on it (ADR 0011 §7).
- #236 (memory ADR, `deferred`) waits for #245. #146 stays open, `deferred` (waits for #25 or #236).
- #21 stays open until the owner's drills (#208) and AC.4 are in docs/retros/0000-pilot.md. Scratch branch
  `t/21-drill-crash` stays (the guard blocks deletes).
- #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).

## Next step
0. `cargo xtask next-session` (TICK §2). It reads the newest *completed* main `ci` run. Expected: rule i, the lowest
   M0 `ready` issue whose `Depends on:` issues are closed (#275 waits for PR #276), else `planner` (rule j, once a
   day) or `idle`.
1. **Next audit:** 1 product merge since audit 0006 (9 merges in all); due at 10 product merges or 2026-11-30.
2. **Next retro (0003): by 2026-11-30.** Measure retro 0002's two action items (0 conflicts in ops/journal/**,
   conflict-only sessions ≤ 1 in 20; PRs at 4 rounds ≤ 10%, no "passes when nothing ran" audit finding), retro 0001 item 1's
   audit lag under the scheduler, the product-merge share (baseline 23%) and the records-only share
   (baseline 24%, #243's measure). Carried inputs: 5 applied proposals
   still say "proposed" (audit 0006); audit 0004 AC.3/AC.4; Codex timeouts and brief-only reviews (PR #268, audits
   0005–0006).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217, #223, #252,
  #263; low #83, #122, #132, #134, #136, #137, #166, #193, #203, #218, #262.

## Blockers / owner-todo
- #245 / PR #276: accept 824f0a4 as is (comment so) or ask for one more final check (on #245).
- #187 / PR #232: accept the final-check wording fix 0831d70 or ask for one more final check (options on #187).
- #260: post an untagged comment if you want #242 then #245 worked first (ADR 0011 §7).
- #266: apply ops/proposals/hold-automerge.md and run T0–T4.
- #257: drop the Playwright cache step in ci.yml (protected).
- #263: decide the two owner-feed rules (edited review bodies; what an endorsing comment pins).
- #196: apply ops/proposals/codex-homebrew-read/codex-review.diff, run T1–T6.
- #179: run test step 3 of ops/proposals/owner-comment-tracking.md.
- #176, #126: driver usage-limit pause; the Codex second-reviewer trial (live).
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- #208: owner fire-drill steps (stranger issue, live unsafe-keyword push, forced judge infra failure, /hold re-run).
- Local XCUITest on the Mac mini needs an unlocked, logged-in console. macOS CI runs no Rust tests.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.

## Pending lessons
- Engine & security or Tooling: a scheduler/tool input built live from command output (git reads) and parsed later →
  put the filtering in a pure function over the command text with a fake reader, and decide per failure (bad bytes,
  oversize) whether it skips, exits 1 or exits 2, before round 1 (PR #285 R1.1:
  [journal](journal/2026-11-23/2225-243-step11.md)).
