# STATUS (≤ 1 page; rewritten only when in-flight work, the next step or blockers change, ops/TICK.md §5)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #276 (#245, ADR 0011 task 3 part 1) merged (139ea44). Part 2: #275 (next-session step 6). Follow-ups: #277.
- `cargo xtask next-session` decides steps 1–5, 7, 10 and 11 (PRs #269, #272, #274, #284, #285); step 6 is checked by
  hand until #275.
- PR #296 (#295): `next-session` skips an own PR on the owner's `/hold` (step 3; step 1 waits for its items too).
  Merged origin/main in (PR #289's step 8), round 3 delta review clean.
- PR #299 (#298, built on PR #296): step 3 also skips an open PR on an `owner/` branch (the owner's own protected
  change, AUTOMATION.md §6.1), so owner-apply PRs #290 and #291 stop being picked. Round 2 delta review clean.
- PR #289 (#244, task 2 part 1: Planner session TICK §9, `ops/QUEUE.md`, step 8) merged (5e4f6f7). Part 2 (work-mix
  budget): #288. Round 5 minors and nit: #294.
- The judge's M0 gate passes (nightly 45179218903, 2026-11-23: `gates.M0.pass`, `current_milestone` M1), but M0 is not
  tagged: its backlog isn't empty (rule j) and PLAN §8's pilot fire drill (#21, owner steps #208) is still open.
- PR #232 (#187 part 1) merged (48f63d9) on the owner's "Merge"; "done:" posted. Part 2: #231 (sev:high, M2,
  `deferred`).
- **#260** (tagged, `human`): work #242 then #245 first. Not an instruction until the owner posts an untagged comment
  on it (ADR 0011 §7).
- #236 (memory ADR, `deferred`) is the first design queue item now that PR #276 merged. #146 stays open,
  `deferred` (waits for #25 or #236).
- #21 stays open until the owner's drills (#208) and AC.4 are in docs/retros/0000-pilot.md. Scratch branch
  `t/21-drill-crash` stays (the guard blocks deletes).
- #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).

## Next step
0. `cargo xtask next-session` (TICK §2). It reads the newest *completed* main `ci` run. Expected: rule i, the lowest
   M0 `ready` issue whose `Depends on:` issues are closed (#246, #249, #275), else `planner` (rule j, once a
   day) or `idle`. The Planner (TICK §9, step 8) is the gate into `ready`.
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
- Process: a PR that adds a session type whose PR closes no issue (Planner) → before round 1, walk its PR through
  every TICK section keyed to an issue (§4 K and cap, §4 step 6 escalation, §5 `/hold` and `owner-todo`, §8) and
  write each mapping down; PR #289 drew one such gap per round (R1.1, R2.2, R3.1, R4.1, R4.2, R5.1–R5.3) and reached
  design ([journal](journal/2026-11-24/0004-244-planner.md)).
- Tooling: a Rust check that mirrors a GitHub Actions `if:` expression → fold case: `startsWith`/`contains` there
  ignore case (PR #296 R1.1: [journal](journal/2026-11-24/0150-295-hold-skip.md)).
- Tooling: a mutant made with `sed -i.bak` and undone with `mv` keeps the old mtime, so cargo reuses the mutated
  binary → `touch` the restored file before any live run ([journal](journal/2026-11-24/0150-295-hold-skip.md)).
