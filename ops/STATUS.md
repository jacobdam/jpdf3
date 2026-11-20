# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #27 (#5 `cargo xtask check-specs` + `specs/TRACE.md`): reviewed (3 rounds), auto-merge on.
  If it is not merged at session start, check its CI first (TICK rule c).

## Last session
- #5: `cargo xtask check-specs` checks feature-spec checklist sections, journey format (a
  stricter copy of the judge's parser) and TRACE completeness. It runs in `cargo xtask ci linux`.
- `specs/TRACE.md` maps PLAN §1 R1–R13 and §4 feature groups (F-RD-1 …) to journeys, specs and
  other tests. **Every new journey and feature spec must be added to TRACE**, or CI fails.

## Next step
- Once #27 merges: check that #5 closed.
- Then the lowest `ready` M0 issue: #6 (journeys 1/4: reader). Journey files must pass
  `cargo xtask check-specs`: run it locally and add each new ID to `specs/TRACE.md`.
- #21 still waits on #12, #13 and #19.
- A cheap open-rate win for later: nested page trees (44 of the 45 local failures).

## Blockers / owner-todo
- Owner: import ops/rulesets/main.json; grant Screen Recording + Accessibility to Terminal
  (macOS UI tests, PDFgear comparison). #21 will need the owner for some drills.
