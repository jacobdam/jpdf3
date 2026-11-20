# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- Nothing open.

## Last session
- main went red after the #39 merge: only the Pages deploy failed (artifact fetch, infra).
  Reran failed jobs → green. PR #40 (#10, UI spec 1/2) merged. #9 labeled `ready`.

## Next step
- #9 (journeys 4/4) is `ready` (#6, #7, #8 merged) → do it next.
  It must also write the "unsaved changes" and "file changed on disk" journeys that
  `specs/ui/information-architecture.md` §8 points to (now "no journey yet; #9"), and the Forms
  and Secure mode details (§3) and Signatures panel (§4).
- #11 (wireframes) depends on #10: use the IA's §10 platform mapping and its states table.
- #21 still waits on #12, #13 and #19.

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
