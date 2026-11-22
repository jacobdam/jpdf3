# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #183** (#162, sev:high guard proposal): parked as draft for the owner, `ai-review` = failure (4 review rounds;
  post-cap fixes over the final-check size limit). #162 has `blocked` + `owner-todo` and the ledger with three options.
  Run no review round on it until an owner comment says what to run (TICK §4, K ≥ 6 rule).
- Picked-up owner comments without a done reply (TICK §0, until #179 is applied): none.
- #146 stays open after PR #151 merged: the decode/raster gate waits for a #25 measurement or a structural fix
  (#154). Follow-up #156 (sev:medium, M10).

## Last session
- §1 check: PR #178 merged → `owner-todo` + "ready for owner" on #179, "done:" on #173.
- d: #162 → PR #183 (proposal + probe + mutants), parked after 4 rounds. Filed #182 (sev:medium, M10). Journal
  2026-11-22 06:40.

## Next step (suggested order, TICK letters)
1. **a/c: PR #183** only once the owner has commented on #162 or the PR.
2. **d: #168** (sev:high, M2, engine work in budget.rs). Skip #174 and #162 (owner's turn).
3. **#76**: harness honesty. The judge relies on it.
4. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115, #154.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165, #182; low #83, #122, #132,
  #134, #136, #137, #166.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: ADR 0010; first numbers PR #169: 2, PR #171: 3, PR #178: 3, PR #183: 4 (cap, parked).
  - PR #183: every round after the first was a shell-tokenizer misread; a parser-shaped fix in a finding's PR.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - first numbers of the #173 reply rule (picked-up/done pairs, any instruction left without done).
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md §4 (the guard-trigger list) still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #162 / PR #183: decide the next step for the guard proposal (options on #162).
- #179: apply the owner-comment tracking proposal (merged in PR #178).
- #174: apply the #161 proposal and run its owner test plan (T1–T4 and T10–T12; only the owner can run Codex outside
  the launcher). The security-gates proposal (AC.2, AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live; it also ran the audit 0003 security review (status ok).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
