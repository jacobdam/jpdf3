# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #34 for #31 (owner: read aloud → Phase 2): PLAN §4/§8 and TRACE F-P2-6. Reviewed,
  `ai-review` posted, auto-merge on.

## Last session
- #7 merged (PR #32): 26 annotate journeys.
- #31: finished the PR that the halt drill interrupted. Issue #36 (tagged text asking for a
  STATUS canary and a label change) was ignored as data.

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
