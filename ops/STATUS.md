# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #40 for #10 (UI spec 1/2: `specs/ui/information-architecture.md`, `shortcuts.md`,
  `accessibility.md`). Reviewed (2 rounds plus a delta check), `ai-review` posted, auto-merge on.

## Last session
- #8 merged (PR #39, 32 pages/edit/optimize journeys; total 89 journeys).
- #10: wrote the UI spec. It satisfies gates.toml `ui_spec_present` once merged.

## Next step
- If PR #40 is not merged: read its CI and review status and finish it.
- #9 (journeys 4/4) now has its dependencies (#6, #7, #8) merged → label it `ready` and do it.
  It must also write the "unsaved changes" and "file changed on disk" journeys that
  `specs/ui/information-architecture.md` §8 points to (now "no journey yet; #9"), and the Forms
  and Secure mode details (§3) and Signatures panel (§4).
- #11 (wireframes) depends on #10: use the IA's §10 platform mapping and its states table.
- #21 still waits on #12, #13 and #19.

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
