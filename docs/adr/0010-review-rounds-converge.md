# 10. PR reviews converge: a severity rubric, delta re-reviews and a 4-round cap

Status: accepted (owner issue #155, "Limit number of review round in PR: think systematically at
process level", 2026-11-22). Supersedes `ops/proposals/review-round-cap.md` (owner request on PR
#151), which proposed only the cap.

## Context
TICK §4 step 5 asked for a fresh re-review whenever a round found a blocker or major, with no
limit. Rule c read "failing CI or review → fix it. Max 3 attempts per PR", so on paper it covered
review too, but no session applied it to review rounds: it named no exit for findings that keep
coming, and §4 step 5 kept demanding re-reviews. Round counts since reviews were first posted on PRs (#124):

| PR | Rounds | Blockers + majors per round (R = subagent, S = Codex) |
|---|---|---|
| #129 | 5 | R: 3, 3, 3, 2, 0 |
| #133 | 6 | R: 0, 0, 0, 1, 0, 0 |
| #135 | 6 | R: 2, 1, 1, 1, 0, 0 |
| #142 | 6 | R: 5, 2, 1, 1, 0, 2 |
| #151 | 15 | R: 0 majors in any round. S: 1 major in **every** round (2 in round 14) |

Earlier PRs (#121: 4 rounds) did not post their reviews. The owner stopped PR #151 by hand after
14 rounds.

Root causes (5 whys, short):
1. **Severity was undefined.** Neither TICK nor the Codex preamble said what a blocker or major
   is. "Not proven", "an estimate could be wrong" and "the best-effort scanner can be bypassed"
   were graded major and forced another round. In PR #151, 16 of 16 majors were of this kind.
2. **Every re-review started from zero.** A fresh reviewer with only the diff re-reviews the whole
   PR, finds whatever it ranks first, and does not know what earlier rounds settled. A reviewer
   that always reports its top finding (Codex in #151: exactly one major per round) never lets
   the loop end.
3. **Fixes grew the scope.** Answering "not proven" with enforcement tooling (the #151 tripwire)
   added new code, and that code drew the next major (rounds 6, 7 and 9 to 14 were about the
   tripwire).
4. **No exit.** Nothing said what happens when findings keep coming, so the only exit was the
   owner.

## Decision (applied in ops/TICK.md §2 rule c, §4, the §5 journal template and §7)
1. **Severity rubric**, given to every reviewer in the brief and applied again by the author at
   triage. Blocker and major need a concrete failing input or scenario caused by the PR (in what
   it adds or changes, or in untouched code whose behavior it changes):
   - blocker: a CONSTITUTION violation, a panic or hang on hostile input, a bypassed limit, a
     security hole, a weakened test assertion the PR doesn't explain;
   - major: wrong behavior or a false statement (spec, comment, PR description) for a concrete
     input, or a missing test for behavior or a limit the PR claims;
   - minor: real but not shown to fail: hardening, an unproven bound that the text already calls
     an estimate, a bypass of best-effort or heuristic tooling, a test that could be stronger, a
     pre-existing problem the PR doesn't cause;
   - nit: wording and style.
2. **Round 1 is a full review; rounds 2 and later are delta reviews.** The brief adds a
   ledger (every earlier finding with its triage line) and the fix diff since the last reviewed
   commit. The delta reviewer checks that each fix works, that the fixes added no blocker or
   major, and whether each dispute or regrade holds. On lines the fixes did not touch, it reports
   blockers only.
3. **Self-grading is always seen by a reviewer.** The author may regrade or dispute a blocker or
   major, but every such regrade goes to a reviewer (the next delta round, or after round 4 the
   final check) before `ai-review` can succeed. That reviewer rules on each one explicitly
   (silence counts as rejected), and a rejection is final.
4. **Second-reviewer-only findings.** A behavioral blocker or major (panic, hang, wrong output,
   bypassed limit) counts once the author reproduces it with a failing test or command; if the
   author can't, it is regraded with the attempt as the reason (and rule 3 sends it to a
   reviewer). A non-behavioral one (CONSTITUTION, license, resemblance to another engine, a false
   statement) is checked against the cited lines and fixed or disputed with evidence.
5. **Review grade and security severity are separate.** A deferred security finding takes its
   `sev:*` label from the ADR 0009 table, not from the review grade. As ADR 0009 says, only
   `sev:medium` / `sev:low` findings may be deferred. This ADR extends ADR 0009 for one case: a finding that is `sev:high` or above by
   the table is fixed in the PR once it reproduces and dropped when a reviewer accepts that it
   isn't real, but when a reviewer accepts that it didn't reproduce, the PR may merge and the
   finding is filed as a `sev:high` issue with the attempt (rule d), so it is tracked, never
   silently lost.
6. **Fix only what was found.** A finding about an unproven property is fixed by stating the
   limitation. Requests for new enforcement tooling (scanners, tripwires, extra checkers) go to
   their own issue.
7. **Cap: 4 rounds per issue** (1 full + 3 delta; the round number K counts headers on every PR
   for the issue, so a replacement PR doesn't reset it). After round 4, minors and nits are
   deferred to issues. Each round-4 blocker or major is fixed (behavior: a test that fails before
   and passes after, both runs recorded; text: the corrected wording quoted), disputed or
   regraded. If
   anything changed after round 4's commit, or a blocker or major is still open or awaiting a
   ruling, and the diff adds ≤ 100 lines and no new file, a **final check** (a fresh reviewer:
   that diff, the ledger entries it concerns, the rubric, CONSTITUTION.md) confirms each fix,
   rules on each dispute and reports blockers or majors the diff adds. Anything it reports, or a bigger fix, sends the PR back to
   draft and the issue to `blocked` for the owner. So a PR sees at most 5 reviews without the
   owner, code written
   after round 4 is still reviewed, and a spurious round-4 finding is settled by a reviewer, not
   the owner. After that, any further round (K ≥ 6) needs an owner comment saying what to run. "Changed
   after round 4" counts only the author's changes, not main merged in.
8. Rule c's "3 attempts" now counts CI fix attempts only. This is a deliberate change: the old
   text also covered review, and 4 rounds is looser than 3. Four, because real blockers and
   majors came in round 2 or later on PRs #129 (rounds 2 to 4), #135 (2 to 4), #142 (2 to 4) and
   #153 (round 2); with a cap of 3, PRs #129, #135 and #142 would have stopped on real findings
   that were then fixed in the PR (except #142's round-3 major, deferred to #146). Delta
   reviews make each extra round cheaper than before. PR #121's round-4 blocker falls inside the
   cap. A cap of 4 still misses something: PR #142's round 6 found 2 real majors (a false
   fragmentation claim and a stale PR description), fixed in the PR. Under this ADR no reviewer
   would have looked for them after round 4, so they would have merged. That is the price of a
   bound. Both were wording errors, which a later PR or audit can still fix; behavior bugs that
   slip through are what the retro metric below counts.
9. The journal's `Cost:` line records rounds, regrades (and how many the next reviewer
   overturned) and deferred issues; the retro computes the metrics below.

## Consequences
- Some real issues merge as deferred minors. ADR 0009 already allows that for medium and low
  security findings; this extends it to non-security findings without a concrete failing
  scenario.
- The author grades their own PR's findings. Three checks keep that honest: the regrade reason
  is posted on the PR; every regrade of a blocker or major goes to a delta reviewer whose grade
  is final; and the owner reads the posts (#124).
- The Codex preamble is protected and doesn't define severity. The rubric reaches Codex through
  the brief, which it reads as data, so it may ignore it. The author's triage (rules 3 and 4) is
  the backstop.

## Success check (next retro, then the one after; computed in TICK §7 step 1)
- Over the next 10 merged PRs: no PR has more than 5 reviews (4 rounds + final check) without an owner comment; median
  rounds ≤ 3; at most 2 of 10 reach the cap.
- Every deferred finding has an issue, and every regrade went to a reviewer. Regrades the next
  reviewer overturned: target ≤ 1 in 4; above that, the author regrades too eagerly and the
  rubric needs tightening.
- The retro re-reads every issue deferred from a review since the last retro and counts the ones
  that were real bugs that should have blocked their merge (target: 0). If any were, tighten the
  rubric instead of raising the cap.
