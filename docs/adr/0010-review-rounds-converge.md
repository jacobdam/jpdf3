# 10. PR reviews converge: a severity rubric, delta re-reviews and a 4-round cap

Status: accepted (owner issue #155, "Limit number of review round in PR: think systematically at
process level", 2026-11-22). Supersedes `ops/proposals/review-round-cap.md` (owner request on PR
#151), which proposed only the cap.

## Context
TICK §4 step 5 asked for a fresh re-review whenever a round found a blocker or major, with no
limit. Rule c's 3-attempt cap was written for CI fixes, and sessions did not apply it to review
rounds. Round counts since reviews were first posted on PRs (#124):

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
   added new code, and that code drew the next major (rounds 6 to 14 were all about the
   tripwire).
4. **No exit.** Nothing said what happens when findings keep coming, so the only exit was the
   owner.

## Decision (applied in ops/TICK.md §2 rule c, §4 and §5)
1. **Severity rubric**, given to every reviewer in the brief and applied again by the author at
   triage. Blocker and major need a concrete failing input or scenario in code, tests or text
   the PR adds or changes:
   - blocker: a CONSTITUTION violation, a panic or hang on hostile input, a bypassed limit, a
     security hole, a weakened test assertion;
   - major: wrong behavior or a false statement (spec, comment, PR description) for a concrete
     input, or a missing test for behavior or a limit the PR claims;
   - minor: real but not shown to fail: hardening, an unproven bound that the text already calls
     an estimate, a bypass of best-effort or heuristic tooling, a test that could be stronger, a
     problem in code the PR does not touch;
   - nit: wording and style.
   The author may regrade a finding that has no concrete scenario, saying why in the triage
   line. The next delta reviewer sees the regrade and can dispute it.
2. **Round 1 is a full review; rounds 2 and later are delta reviews.** The brief adds a
   ledger (every earlier finding with its triage line) and the fix diff since the last reviewed
   commit. The delta reviewer checks that each fix works, that the fixes added no blocker or
   major, and whether each dispute or regrade holds. On lines the fixes did not touch, it reports
   blockers only.
3. **A second-reviewer-only blocker or major triggers a re-review only when the author reproduces
   it** (a failing test or command). If the author can't reproduce it, it is regraded minor and
   deferred.
4. **Fix only what was found.** A finding about an unproven property is fixed by stating the
   limitation. Requests for new enforcement tooling (scanners, tripwires, extra checkers) go to
   their own issue.
5. **Cap: 4 rounds per PR** (1 full + 3 delta). Findings still open after round 4:
   - minors and nits are deferred to issues;
   - a blocker or major is fixed with a test that fails before the fix and passes after it, both
     results recorded in the triage (a false statement: the corrected text is quoted), with no
     further review round. If the fix is more than a
     local change, the PR goes back to draft, the issue is labeled `blocked` with the ledger, and
     the owner decides.
6. Rule c's "3 attempts" counts CI fix attempts only. Review rounds follow this cap.
7. The journal's `Cost:` line records `review rounds N/4` and the deferred issues, so the retro
   can measure this.

## Consequences
- Some real issues merge as deferred minors. ADR 0009 already allows that for medium and low
  security findings; this extends it to non-security findings without a concrete failing
  scenario.
- The author grades their own PR's findings. Three checks keep that honest: the regrade reason
  is posted on the PR, the next delta reviewer can dispute it, and the owner reads the posts
  (#124).
- The Codex preamble is protected and doesn't define severity. The rubric reaches Codex through
  the brief, which it reads as data, so it may ignore it. The author's triage (rule 3) is the
  backstop.

## Success check (next retro, then the one after)
- Over the next 10 merged PRs, no PR goes past 4 review rounds.
- Every deferred finding has an issue.
- Retro: of the findings deferred under this ADR, how many later turned out to be real bugs that
  should have blocked the merge (target: 0)? If any did, tighten the rubric instead of raising the
  cap.
