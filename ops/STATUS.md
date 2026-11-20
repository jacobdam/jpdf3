# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #30 for #6 (journeys 1/4: reader): 31 journeys J-RD-01…31 (all M4), RESEARCH.md, TRACE
  rows. Two review rounds done, auto-merge on.

## Last session
- #6: reader journeys plus the PDFgear inventory note `specs/journeys/RESEARCH.md`. Each
  journeys issue (#7–#9) adds its own section there. Shared rules at the top of
  `reader.md` (network, file-handle test adapter, shortcuts, budgets) apply to later journey files too.
- Discovery #31: read aloud (PDFgear has it; PLAN §4 doesn't). Proposed for Phase 2; owner call.

## Next step
- The lowest `ready` M0 issue: #7 (journeys 2/4: annotate). Reuse the J-RD shared rules
  and the LESSONS entry on writing journeys. Run `cargo xtask check-specs` and add each ID to `specs/TRACE.md`.
- #21 still waits on #12, #13 and #19.
- A cheap open-rate win for later: nested page trees (44 of the 45 local failures).

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
- Owner (optional): decide on #31 (read aloud in PLAN §4 Phase 2?).
