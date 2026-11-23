# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **#260** (filed by an owner interactive session, tagged, `human`): work #242 then #245 ahead of the other M0
  `ready` issues. Not an instruction until the owner posts an untagged comment on it (ADR 0011 §7). Once that
  comment exists: rule a, start with #242.
- **Audit 0006** (#258, PR #259): docs/audits/0006.md. New: #261 (M0 bug: metrics edge cases, fuzz exit-status test,
  capture 16 MiB boundary), #262 (sev:low, M10), #263 (sev:medium, M10: owner feed misses edited review bodies; the
  ADR 0011 §7 endorsement isn't tied to the body version; both need an owner decision). #223 now sev:medium, M10.
- **PR #232** (#187 part 1): parked for the owner (draft, `ai-review` failure on 0831d70 after 4 rounds + final
  check). No review round until an owner comment. Part 2: #231 (sev:high, M2; audit 0006 AS.6 added there).
- #254 done: the owner applied ops/proposals/ci-fuzz-tools-cache.md in #256. #257 (`owner-todo`): drop the Playwright
  cache. #252 (sev:medium, M10), #253 (nit) remain.
- ADR 0011 (#238) accepted; tasks #242–#251. #236 (memory ADR) waits for #245 (owner direction on #236).
- #146 stays open (decode/raster gate; waits for #25 or #236). #235 is the dependency gate's follow-up.
- #21 stays open until the owner's drills (#208) and AC.4 are in docs/retros/0000-pilot.md. The /hold drill fails:
  #210. Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- g: audit 0006 (PR #259). Journal 2026-11-23 16:19.

## Next step (suggested order, TICK letters)
0. **a: #260** (#242, then #245) once the owner has commented on it untagged; else rule i, lowest number first:
   #210, #216, #219, #235, #242, #261.
1. **a/c: PR #232** only once the owner has commented on #187 or the PR.
2. When #208 has the owner's results: add them and AC.4 to docs/retros/0000-pilot.md and close #21.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217, #223, #252,
  #263; low #83, #122, #132, #134, #136, #137, #166, #193, #203, #218, #262.
- **Next retro (rule h): by 2026-11-28.** Start from `git fetch` + `cargo xtask metrics --index <main checkout>/ops/logs/index.tsv`
  (specs/tooling/metrics.md; #261 fixes a negative retro-lag line for past periods). Inputs carried from the last
  STATUS:
  - review rounds (ADR 0010): PR #169: 2, #171: 3, #178: 3, #183: 4 (cap, parked), #186: 4 + final check, #198: 2,
    #201: 2, #204: 3, #209: 2, #212: 3, #215: 2, #220: 3, #227: 4 + final check, #232: 4 + final check (parked),
    #234: 4 + final check, #241: 4. PR #183: every later round was a shell-tokenizer misread.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - first numbers of the #173 reply rule (picked-up/done pairs).
  - the template's `Area:` list lacks `tooling`; ops/TICK.md §4 still cites guard-inert-post-text.md as pending
    (applied in #145).
  - audit 0004 AC.3 and AC.4 (ops/TICK.md:29, AUTOMATION.md:295).
  - Codex on audits: audits 0005 and 0006 ran ok but brief-only (workspace commands timed out).
  - ops/LESSONS.md has 63 entries (cap 60).
  - audits 0004–0006: gates that pass when nothing ran ("what still passes"), now in #262 and #261.
- **Next audit:** after 10 more merges or by 2026-11-30.

## Blockers / owner-todo
- #260: post an untagged comment if you want #242 then #245 worked first (ADR 0011 §7).
- #257: drop the Playwright cache step in ci.yml (protected).
- #263: decide the two owner-feed rules (edited review bodies; what an endorsing comment pins).
- #187 / PR #232: accept the final-check wording fix 0831d70 or ask for one more final check (options on #187).
- #196: apply ops/proposals/codex-homebrew-read/codex-review.diff, run T1–T6.
- #179: PR #184 merged; run test step 3 of ops/proposals/owner-comment-tracking.md (a PR comment and an inline
  review comment; check that the next session replies picked up and done:).
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #208: owner fire-drill steps (stranger issue, live unsafe-keyword push, forced judge infra failure, /hold re-run
  after #210).
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
