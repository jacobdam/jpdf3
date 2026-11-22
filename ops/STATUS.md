# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #151 (#146, sev:high): on owner `/hold`. Draft, `ai-review` = success on f3a22d0. Do not mark it ready or
  enable auto-merge until the owner comments `/release`; then enable auto-merge.**
  - Follow-ups: #154 (enforce the gate structurally), #156 (sev:medium, M10). #146 stays open.
  - The owner's `/halt` on PR #151 isn't a defined command (no `halt` label, no `ops/HALT`).
- **PR #157 (#155, owner issue): draft, #155 `blocked`, `ai-review` = failure on 6e799c2, waiting for the owner.**
  Act only on an owner comment on #155 or PR #157: accept, run one more final check, or change.
- PR #160 (audit 0003, #159): this session's record. If it hasn't merged when you read this, check its CI.

## Last session
- Audit 0003 (rule g, 20 merges since audit 0002), PR #160, record docs/audits/0003.md. Three subagent reviews plus
  the first Codex audit review (34 findings in all), all posted on PR #160. New issues: #161 and #162 (sev:high,
  owner-todo, protected files: Codex launcher isolation, guard tag check), #168 (sev:high, M2: JBIG2 page data +
  globals budget), #163, #164, #165 (sev:medium, M10), #166 (sev:low, M10); #87 comment; proposal ops/proposals/security-gates-in-gate-tool.md. Journal 2026-11-22 03:10.

## Next step (suggested order, TICK letters)
0. PR #151: wait for `/release`. PR #157: wait for an owner comment. Don't touch either before.
   Until #157 merges, TICK §4 on main is still the old loop, with no defined exit.
1. **d: #161, then #162** (sev:high). Both are in protected files, so the work is a proposal each
   (`ops/proposals/codex-review-isolation.md`, `ops/proposals/guard-tag-and-fail-closed.md`) with the diff and a
   test plan; label `owner-todo` stays. #162's CX.4 is confirmed and small (drop the raw-text prefilter).
   #168 (sev:high, M2) is engine work in budget.rs and can go before or after them.
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115, #154.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- Security issues waiting in M10: medium #114, #116, #117, #141, #156, #163, #164, #165; low #83, #122, #132, #134,
  #136, #137, #166.
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: owner issue #155 / PR #157 (ADR 0010) once it merges; the retro checks its first numbers.
  - STATUS "in flight" goes stale as soon as its PR merges.
  - the template's `Area:` list lacks `tooling`.
  - ops/TICK.md:204 still cites ops/proposals/guard-inert-post-text.md as pending (applied in #145).
- **Next audit:** after 10 more merges or by 2026-11-29.

## Blockers / owner-todo
- #161, #162: protected-file security fixes need the owner (proposals coming). The security-gates proposal (AC.2,
  AS.56) needs an owner decision.
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live; it also ran the audit 0003 security review (status ok).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
