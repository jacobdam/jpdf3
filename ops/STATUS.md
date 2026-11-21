# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- Audit 0001 PR (#74): docs/audits/0001.md, journal, STATUS, LESSONS.

## Last session
- **Audit 0001** (#74; the first one, covering 24 merges up to 3b6101b). There were 0 critical findings. The hostile-input
  core is sound: no panics, budgets apply everywhere, no unsafe. The high findings:
  - the conformance suites accept proven-broken adapters (#80, #81, #68)
  - the E2E harnesses can write `pass` for a failed run (#76)
  - a flaky bench test (#75)

  Mutants on the parsers: 81% caught; no limit has an exact-boundary test (#87). Completeness gaps went to journeys
  issues #84 (platform conventions) and #85 (PDFgear inventory). The other issues: #77, #79, #78 (M1), #82, #83, #86;
  nightly no-op → comment on #19.
- Before that: #54 (Rasterizer trait + suite) merged as PR #71.

## Next step (suggested order)
1. **#75 flaky** (rule e; fix within 5 sessions).
2. **#76**: harness honesty, which the judge relies on.
3. **#80, #81, #68**: tighten the suites **before** #16 admits any adapter (each lists the proving toy).
4. **#77, #79**: memory margins and trait-contract budgets (also before #16).
5. Then the `ready` backlog, lowest number first: #55, #61, #64, #66, #70, then #82, #83, #84–#87.
   #14 closes after #55, #20 after #61, #22 after #64.
- #19 (nightly): wire `fuzz::run` and `bench::run` first; meanwhile print "not implemented" on stderr.
- A **retro** is due soon (7 days since bootstrap on 2026-11-27, or earlier on an incident). Agenda item: TICK §2 puts g
  before i, so audits never come up while ready issues exist.
- Next audit: after 10 more merges or by 2026-11-28. It covers PR #71 onward, plus mutants on jpdf-traits, tools and xtask.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools. A local worktree `t/50-test-only-licenses` holds uncommitted
  CONSTITUTION edits (presumably the owner's); agents leave it alone.
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #60: may the Arlington TSVs (and tables generated from them) be committed?
- #21: the owner's fire-drill steps.
- Proposal for the owner: ops/proposals/pages-deploy-retry.md (ci.yml is protected).
- The stale remote branch `t/2-bootstrap-labels` (PR #3, merged) can't be deleted by agents (the guard blocks remote deletes).
