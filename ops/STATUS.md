# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #220** (#91, `cargo xtask metrics` 1/2): `ai-review` = success on 681571f after 3 rounds, auto-merge on. Part 2
  (CI failures by job, metrics.json) is #219.
- **PR #186** (#168, sev:high, JBIG2 page data + globals share one input allowance): parked as draft for the owner,
  `ai-review` = failure (4 rounds + final check; the final check found one false sentence, corrected in 1533e26,
  unreviewed). #168 has `blocked` + `owner-todo` and the ledger with two options. No review round until an owner comment.
- **PR #183** (#162, sev:high guard proposal): parked as draft for the owner (same rule).
- PR #184 (owner's own: applies the owner-comment tracking proposal, #179): open. Audit 0005 found the proposal's
  filter and the guard disagree on leading U+001C–U+001F (#217) and the harness lacks leading-whitespace agent cases
  (#216); heads-up posted on #179. Until it merges, TICK §0's interim rules for owner comments apply.
- Picked-up owner comments without a done reply (TICK §0, until #179 is applied): none.
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).
- #21 stays open until the owner's drills (#208) and AC.4 (comment on #21) are added to docs/retros/0000-pilot.md.
  The /hold drill **fails**: #210 (protected). Scratch branch `t/21-drill-crash` stays (the guard blocks deletes).

## Last session
- i: #91 → PR #220 (`cargo xtask metrics`: throughput, review rounds/findings from posted reviews, stuck issues,
  audit/retro lag; split, part 2 = #219). 3 review rounds (R1: 3 majors, all fixed). Audit 0005 (PR #215) merged
  before the pick. Journal 2026-11-22 12:15.

## Next step (suggested order, TICK letters)
1. **a/c: PR #186 and PR #183** only once the owner has commented on #168/#162 or the PR.
2. #187 waits on PR #186.
3. Rule i, M0, lowest number first: #115, #154, #210 (write ops/proposals/ for the owner-commands fix), #216
   (do it before the owner merges PR #184 if possible: it is the harness the owner's test plan relies on), #219.
4. When #208 has the owner's results: add them and AC.4 to docs/retros/0000-pilot.md and close #21.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182, #217; low #83, #122,
  #132, #134, #136, #137, #166, #193, #203, #218.
- **Next retro (rule h): by 2026-11-28.** Start from `git fetch` + `cargo xtask metrics --index
  <main checkout>/ops/logs/index.tsv` (specs/tooling/metrics.md; CI counts need #219). Inputs:
  - review rounds: ADR 0010; first numbers PR #169: 2, PR #171: 3, PR #178: 3, PR #183: 4 (cap, parked), PR #186: 4 +
    final check (parked on one sentence), PR #198: 2, PR #201: 2, PR #204: 3, PR #209: 2, PR #212: 3.
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
  - ops/LESSONS.md has 63 entries (cap 60).
  - audit 0005: harness tests named for a limit that fail for another reason (AS.3), and proposal harnesses without
    near-miss cases (AC.1; audit 0004 AC.1 was the same pattern in the same harness).
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #196: apply ops/proposals/codex-homebrew-read/codex-review.diff, run T1–T6.
- #168 / PR #186: accept the final-check wording fix or ask for one more round (options on #168).
- #162 / PR #183: decide the next step for the guard proposal (options on #162).
- #179: merge PR #184 (see #216, #217 first), then run test step 3 of ops/proposals/owner-comment-tracking.md.
- The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #208: owner fire-drill steps (stranger issue, live unsafe-keyword push, forced judge infra failure, /hold re-run
  after #210).
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live.
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
