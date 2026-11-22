# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #239** (#238, ADR 0011 session types): ready, `owner-todo`, **no auto-merge**: waits for the owner's approval.
  4 rounds + final check. After it merges: file the ADR §9 tasks (1a first) and link them on #238, close #224 into
  task 3, then post "done:" on #238. #236 waits for task 3 (design: technical).
- **PR #232** (#187 part 1, `ImageInputBudget::charge_output`): **parked for the owner**: draft, `ai-review` =
  failure on 0831d70 after 4 rounds + final check. The final check's Codex half found one false spec sentence (TIFF
  predictor tag bytes), corrected in 0831d70 unreviewed. #187 has `blocked` + `owner-todo` and the ledger with two
  options. No review round until an owner comment. Part 2: #231 (sev:high, M2, needs the image-decode caller).
- Follow-ups of PR #227 (#115): #225 (M1), #228 (M10), #229 (M1). #223 and #224 came out of the closed PR #183.
- PR #184 (owner's own, the owner-comment tracking feed) **merged** (66f4d3c): `owner-issues.sh` now lists comments
  from 2026-11-23 on until a "done:" reply links them. #179 keeps `owner-todo` for the owner's test step 3. #216 and
  #217 still apply to the applied script.
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).
- #21 stays open until the owner's drills (#208) and AC.4 (comment on #21) are added to docs/retros/0000-pilot.md.
  The /hold drill **fails**: #210 (protected). Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- a: #238 → PR #239 (ADR 0011: session types, read-only scheduler `cargo xtask next-session`, product-merge
  counters, area labels and design-pending, review-cap escalation to design). Journal 2026-11-22 16:24.

## Next step (suggested order, TICK letters)
0. **a: #238** is the owner's focus: once PR #239 merges, file its §9 tasks and work them in order (1a first).
1. **a/c: PR #232** only once the owner has commented on #187 or the PR (accept 0831d70, or one more final check).
   #231 isn't workable until the M2 image-decode caller exists.
2. Rule i, M0, lowest number first: #154, #210 (write ops/proposals/ for the owner-commands fix), #216
   (the harness the owner's test step 3 relies on; PR #184 already merged), #219.
3. When #208 has the owner's results: add them and AC.4 to docs/retros/0000-pilot.md and close #21.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217; low #83, #122,
  #132, #134, #136, #137, #166, #193, #203, #218.
- **Next retro (rule h): by 2026-11-28.** Once PR #220 is on main, start from `git fetch` + `cargo xtask metrics --index
  <main checkout>/ops/logs/index.tsv` (specs/tooling/metrics.md; CI counts need #219). Inputs:
  - review rounds: ADR 0010; first numbers PR #169: 2, PR #171: 3, PR #178: 3, PR #183: 4 (cap, parked), PR #186: 4 +
    final check (parked on one sentence), PR #198: 2, PR #201: 2, PR #204: 3, PR #209: 2, PR #212: 3, PR #215: 2,
    PR #220: 3, PR #227: 4 + final check (every Codex round found
    a new test-strength gap in a widened file-tree scan), PR #232: 4 + final check (parked on one sentence; all majors
    after round 1 but one were spec text about a future caller).
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
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #238 / PR #239: approve (or comment on) ADR 0011; the PR has no auto-merge.
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
