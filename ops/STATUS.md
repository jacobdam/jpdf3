# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #227 (#115) merged at 13:51 (830936d). Follow-ups #225 (M1), #228 (M10), #229 (M1).
- The owner merged PR #186 (#168, JBIG2 input allowance, after a round-6 check) and closed PR #183 unmerged (#162,
  guard proposal; follow-ups #223 security by isolation, #224 architect role), both at 12:31. #168 and #162 are
  closed. #187 (sev:high, M2) builds on PR #186 and is now workable (rule d).
- PR #184 (owner's own, the owner-comment tracking feed) **merged** (66f4d3c): `owner-issues.sh` now lists comments
  from 2026-11-23 on until a "done:" reply links them. #179 keeps `owner-todo` for the owner's test step 3. #216 and
  #217 still apply to the applied script.
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).
- #21 stays open until the owner's drills (#208) and AC.4 (comment on #21) are added to docs/retros/0000-pilot.md.
  The /hold drill **fails**: #210 (protected). Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- i: #115 → PR #227 (new crate tools/fsio for the open flags; Arlington not-shipped scan over whole trees with blob
  ids, symlink and root checks; typed arlington commands). 4 review rounds + final check, Codex ok on all 5. Journal
  2026-11-22 13:55.

## Next step (suggested order, TICK letters)
1. **d: #187** (sev:high, decoded image input buffers on ImageInputBudget from PR #186; read its deferred items).
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
    a new test-strength gap in a widened file-tree scan).
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
