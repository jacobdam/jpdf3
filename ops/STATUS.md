# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #93 (#68): the font suite is tightened (auto-merge on). It includes journal, STATUS and LESSONS.

## Last session
- **#68 font conformance** (rule d). Hostile coverage needs proof: per container+format pairing, the program loads as
  its declared format and a control glyph outlines.
  - New invariants for promised empty answers (cmap/CID).
  - `min_ops` lower bounds with `HEAVY_OPS` 100 per pairing; TrueType needs a composite.
  - The sfnt-CFF pairing is required, and names work past glyph 64.
  - Admission (#16) font fixtures must now supply all of this.
- Before that: Retro 0001 (#90). TICK §2 reordered (audit g, retro h before the ready backlog i), plus a sizing rule
  (≤ 1500 added lines) and a spec-template self-check.

## Next step (suggested order, new TICK letters)
1. **#75 flaky** (rule e; it must be fixed within 5 sessions of audit 0001, and 2 have passed, so take it next even though d comes first).
2. d (security) issues, lowest number first: **#77, #79, #80, #81, #82, #83**. Tighten the suites (#80, #81)
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
- **Next audit (rule g):** at the 10th merge after e82b6a6 (#90 and #93 count), or by 2026-11-28.
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
