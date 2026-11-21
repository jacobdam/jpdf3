# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- Retro 0001 PR (#90): docs/retros/0001.md, TICK, template, LESSONS, proposal, journal, STATUS.

## Last session
- **Retro 0001** (owner request #89), docs/retros/0001.md. Numbers: 27 sessions; 26 agent PRs; 58% of substantive PRs
  needed more than 1 fix round; 42% went past the 60% context guideline; the first audit came after 26 merges instead of 10.
- Three action items are applied (retro 0002 judges each keep, revert or extend):
  1. **TICK §2 reordered:** audit = g and retro = h now come before the ready backlog (i) and the gate check (j). §1
     prints merges since the last audit and the last retro's date.
  2. **Sizing rule (TICK §0):** one issue = one PR = at most one trait family, tool or crate plus its suite, and
     ≤ 1500 added lines. Split before claiming.
  3. **Spec template self-check (Security/Limits):** checked arithmetic, "what broken implementation still passes?",
     exact-boundary tests, step caps. The reviewer prompt asks the same questions.
- LESSONS went from 36 to 31 entries.
- #21 (fire drill) is now `ready` (no longer depends on #19).
- New issues: #91 (`cargo xtask metrics`) and digest #92.
- New proposal: ops/proposals/automation-priority-order.md.

## Next step (suggested order, new TICK letters)
1. **#75 flaky** (rule e; fix within 5 sessions).
2. d (security) issues, lowest number first: **#68, #77, #79, #80, #81, #82, #83**. Tighten the suites (#80, #81, #68)
   and the contracts (#77, #79) **before** #16 admits any adapter.
3. **#76**: harness honesty. The judge relies on it.
4. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, then #84–#87, #91.
   - #14 closes after #55 and #70.
   - #20 closes after #61.
   - #22 closes after #64.
   - Split anything over the sizing rule first.
- #16 (admission): every adapter passes its `conformance` suite and adds a fuzz target with seeds. Font adapters also
  need hostile programs per format and a multi-FD CID fixture.
- #19 (nightly): wire `fuzz::run` and `bench::run`.
- **Next audit (rule g):** at the 10th merge after e82b6a6, or by 2026-11-28.
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md, ideally with #91's numbers.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools. A local worktree `t/50-test-only-licenses` holds uncommitted
  CONSTITUTION edits (presumably the owner's); agents leave it alone.
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #60: may the Arlington TSVs (and tables generated from them) be committed?
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- Proposals for the owner:
  - ops/proposals/pages-deploy-retry.md
  - ops/proposals/test-only-licenses.md
  - ops/proposals/automation-priority-order.md
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
