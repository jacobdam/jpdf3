# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #32 for #7 (journeys 2/4: annotate): 26 journeys J-ANN-01…26 (25 at M5, one at M6),
  RESEARCH.md section, TRACE rows. Two review rounds plus a delta check; `ai-review` posted,
  auto-merge on.

## Last session
- #6 merged (PR #30): 31 reader journeys and `specs/journeys/RESEARCH.md`.
- #7: annotate journeys. Shared rules at the top of `annotate.md` (strict writing, flags per
  kind, counting rule, "save and verify" persistence check, hostile text/image sets) apply to
  later journey files that change documents. J-ANN-03 requires an ADR for the QuadPoints
  point order before M5 implementation.

## Next step
- The lowest `ready` M0 issue: #8 (journeys 3/4: pages, edit, optimize, convert). Reuse the
  J-RD and J-ANN shared rules and the LESSONS entry on writing journeys. Run
  `cargo xtask check-specs` and add each ID to `specs/TRACE.md`.
- #8 must also cover: document WS/DS (save) actions being inert (J-RD-28 hand-off), and the
  link tool (insert a link on an area or text) and bookmark editing from RESEARCH.md.
- #21 still waits on #12, #13 and #19.
- A cheap open-rate win for later: nested page trees (44 of the 45 local failures).

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
- Owner (optional): decide on #31 (read aloud in PLAN §4 Phase 2?).
