# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #269 (#242, ADR 0011 task 1a) merged (959c4c6): `cargo xtask next-session` decides TICK rules a–f and prints
  `legacy` otherwise; ops/TICK.md §2 runs it. `deferred` on #236, #146 and #231 so the scheduler skips them.
  **PR #272** (#270) merged: it now picks a conflicting own PR. PR #265 (#216) merged (c863a0d).
- main CI run 45249100863 (64905cf): `project-macos` never got a hosted runner (infra). The rerun was cancelled by the
  main concurrency group (no job ran); 959c4c6's run 45254461191 has `project-macos` green instead.
- Leftover worktrees `t/219-ci-metrics`, `t/235-target-paths`, `t/242-next-session`, `t/270-conflicting-pr` can be
  removed (merged PRs; the scheduler's `Notes:` list them).
- PR #264 (#210) merged; #266 (`owner-todo`): the owner applies ops/proposals/hold-automerge.md and runs T0–T4.
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
- ADR 0011 (#238) accepted; tasks #242–#251. #236 (memory ADR, `deferred`) waits for #245 (owner direction on #236).
- #146 stays open, `deferred` (decode/raster gate; waits for #25 or #236). #235 (PR #268) is the dependency gate's follow-up.
- #21 stays open until the owner's drills (#208) and AC.4 are in docs/retros/0000-pilot.md. The /hold drill fails:
  #210 (proposal in PR #264). Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- c by hand (scheduler said `legacy`; #272 had not merged when this session started): PR #271 (rule b records)
  conflicts with main fixed after PR #272 merged. Journal 2026-11-23 20:26.
- Before it, c: #270 → PR #272 (merged). Journal 2026-11-23 20:12. Before that, b: main red on run 45249100863,
  macOS runner not acquired → rerun (PR #271). Journal 2026-11-23 20:03.

## Next step (suggested order, TICK letters)
0. Start with `cargo xtask next-session` (TICK §2). It reads the newest *completed* main `ci` run, so while newer
   runs are still in progress it can pick an older red run that a newer run may fix; check
   `gh run list --branch main` first.
   On `legacy`: rule i, lowest number first: #243 (task 1b) or #245 (task 3; both need only #242), then #261. #260
   (tagged) asks for #245 next once the owner comments on it untagged.
1. **a/c: PR #232** only once the owner has commented on #187 or the PR.
2. When #208 has the owner's results: add them and AC.4 to docs/retros/0000-pilot.md and close #21.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217, #223, #252,
  #263; low #83, #122, #132, #134, #136, #137, #166, #193, #203, #218, #262.
- **Next retro (rule h): by 2026-11-28.** Start from `git fetch` + `cargo xtask metrics --index <main checkout>/ops/logs/index.tsv`
  (specs/tooling/metrics.md; now with CI failures by job and target/metrics/metrics.json; #261 fixes a negative
  retro-lag line for past periods). Inputs (carried from the
  last STATUS, plus audit 0006):
  - review rounds: ADR 0010; first numbers PR #169: 2, PR #171: 3, PR #178: 3, PR #183: 4 (cap, parked), PR #186: 4 +
    final check (parked on one sentence), PR #198: 2, PR #201: 2, PR #204: 3, PR #209: 2, PR #212: 3, PR #215: 2,
    PR #220: 3, PR #227: 4 + final check (every Codex round found
    a new test-strength gap in a widened file-tree scan), PR #232: 4 + final check (parked on one sentence; all majors
    after round 1 but one were spec text about a future caller), PR #234: 4 + final check (each Codex round found a
    new "what still passes" gap in a parser of cargo's text output or in how the flags were pinned), PR #241: 4 (4
    of 5 majors were prose about actions/cache save/restore behavior in a proposal, at least one in each of rounds 1–3).
  - PR #183: every round after the first was a shell-tokenizer misread; a parser-shaped fix in a finding's PR.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - first numbers of the #173 reply rule (picked-up/done pairs, any instruction left without done).
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md §4 (the guard-trigger list) still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
  - audit 0004 AC.3 (no rule for a CI fix after a clean final check) and AC.4 (a proposal PR's `Closes #N` drops a
    sev:high issue from rule d before the fix is on main, and the owner's new issue isn't told to keep
    `security`/`sev:*`) (ops/TICK.md:29,
    AUTOMATION.md:295).
  - Codex on audits: audit 0005's brief (no ops/driver diff) ran ok (7 findings, 0 Codex-only bugs above sev:low),
    but like PR #212's 3 runs it said its workspace commands timed out (brief-only review).
  - ops/LESSONS.md has 63 entries (cap 60); PR #221 merged the new date lesson into an existing one.
  - audit 0005: harness tests named for a limit that fail for another reason (AS.3), and proposal harnesses without
    near-miss cases (AC.1; audit 0004 AC.1 was the same pattern in the same harness).
  - audits 0004–0006: gates that pass when nothing ran ("what still passes"), now in #262 and #261.
  - Codex on audit 0006: ok but brief-only again; 6 Codex-only findings, none above sev:low or minor.
  - Codex on PR #268 rounds 1 and 2: unavailable, timeout after 2400 s with an empty log (56 KB and 31 KB briefs).
  - ops/proposals/: 5 applied proposals (including owner-comment-tracking.md) still say "proposed" in their status line (docs/audits/0006.md).
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
