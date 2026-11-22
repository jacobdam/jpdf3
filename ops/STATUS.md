# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #186** (#168, sev:high, JBIG2 page data + globals share one input allowance): parked as draft for the owner,
  `ai-review` = failure (4 rounds + final check; the final check found one false sentence, corrected in 1533e26,
  unreviewed). #168 has `blocked` + `owner-todo` and the ledger with two options. No review round until an owner comment.
- **PR #183** (#162, sev:high guard proposal): parked as draft for the owner (same rule).
- PR #184 (owner's own: applies the owner-comment tracking proposal, #179): open; the owner merges it, then runs test
  step 3. Until it merges, TICK §0's interim rules for owner comments apply.
- Picked-up owner comments without a done reply (TICK §0, until #179 is applied): none.
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).

- **PR #209** (#21 part 1/2, pilot fire drill record docs/retros/0000-pilot.md): `ai-review` = success (2 rounds),
  auto-merge on. It says `Refs #21`, not `Closes`: #21 closes once the owner's drills (#208) are added.
  The /hold drill **fails**: #210 (owner-commands can't turn off auto-merge and says it did; protected).
- Scratch drill PR #207 closed unmerged; its remote branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- i: #21 → drills (crash PR #207, weakened-test reviewer, guard probes), record PR #209, owner part #208, bug #210.
  PR #204 and #206 merged. Removed worktrees e2e-honesty, e2e-honesty-2, record-200;
  left pilot-drill (PR #209) and record-21 (this record) until they merge. Journal 2026-11-22 10:30.

## Next step (suggested order, TICK letters)
1. **a/c: PR #186 and PR #183** only once the owner has commented on #168/#162 or the PR.
2. #187 waits on PR #186.
3. **#210** (`ready`, M0): write ops/proposals/ for the owner-commands permission fix (not owner-todo until the
   proposal merges). Then the `ready` backlog (rule i) in M0, lowest number first: #86, #91, #115, #154.
4. When #208 has the owner's results: add them to docs/retros/0000-pilot.md and close #21.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182; low #83, #122,
  #132, #134, #136, #137, #166, #193, #203.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: ADR 0010; first numbers PR #169: 2, PR #171: 3, PR #178: 3, PR #183: 4 (cap, parked), PR #186: 4 +
    final check (parked on one sentence), PR #198: 2, PR #201: 2, PR #204: 3, PR #209: 2.
  - PR #183: every round after the first was a shell-tokenizer misread; a parser-shaped fix in a finding's PR.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - first numbers of the #173 reply rule (picked-up/done pairs, any instruction left without done).
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md §4 (the guard-trigger list) still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
  - audit 0004 AC.3: no rule for a CI fix after a clean final check (K = 6 is the owner's call, but nothing adds
    `owner-todo`); AC.4: a proposal PR's `Closes #N` drops a sev:high issue from rule d before the fix is on main,
    and the owner's new issue isn't told to keep `security`/`sev:*` (ops/TICK.md:29, AUTOMATION.md:295).
  - Codex refused the audit 0004 security brief (provider cyber filter); decide how audits use it.
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #196: apply ops/proposals/codex-homebrew-read/codex-review.diff, run T1–T6.
- #168 / PR #186: accept the final-check wording fix or ask for one more round (options on #168).
- #162 / PR #183: decide the next step for the guard proposal (options on #162).
- #179: merge PR #184, then run test step 3 of ops/proposals/owner-comment-tracking.md.
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #208: owner fire-drill steps (stranger issue, live unsafe-keyword push, forced judge infra failure, /hold re-run
  after #210).
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live; audit 0003's review ran (ok), audit 0004's was refused by the
  provider's cybersecurity filter.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
