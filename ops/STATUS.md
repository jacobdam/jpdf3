# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR for #28 (owner: clear stale owner-todos, scratchpad lesson): docs-only, auto-merge on.

## Last session
- #28 (owner issue): removed owner setup steps the owner finished (ruleset, Pages, Screen
  Recording + Accessibility, commit signing) from the blockers; added the scratchpad lesson.
- Earlier: #5 merged (PR #27). `cargo xtask check-specs` runs in `cargo xtask ci linux`.
  **Every new journey and feature spec must be added to `specs/TRACE.md`**, or CI fails.

## Next step
- The lowest `ready` M0 issue: #6 (journeys 1/4: reader). Journey files must pass
  `cargo xtask check-specs`: run it locally and add each new ID to `specs/TRACE.md`.
- #21 still waits on #12, #13 and #19.
- A cheap open-rate win for later: nested page trees (44 of the 45 local failures).

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
