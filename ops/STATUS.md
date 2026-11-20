# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #56 (#14, part 1 of 4): `Limits` moved into `jpdf-traits`, plus `DecodeLimits`,
  `BackendError`, the Flate, JPEG, JPEG 2000, JBIG2, CCITT, hash and cipher traits, and
  the `jpdf_traits::conformance` suites.
  - Spec: specs/core/backend-traits.md. ADR: 0004.
  - Reviewed (approved after one fix round plus a delta review). Auto-merge is on.
  - #14 stays open until parts 2–4 (#53, #54, #55) land. Close it after #55.

## Last session
- Split #14 into #14, #53, #54 and #55. Implemented part 1.
- Opened discovery #57: JPX components with mixed bit depths currently return
  `Unsupported`. Decide before the first JPX adapter (#16).

## Next step
- `ready` M0 issues: #17, #18, #20, #22. Then #53, #54, #55, all unblocked once #56 merges;
  give them the `ready` label then.
- #16 (backend admission) can start once #56 merges. Every adapter must pass its
  `jpdf_traits::conformance` suite. JPEG, JPX, JBIG2 and CCITT adapters must supply our
  own fixtures: a landscape and a portrait image, and for JPX also a fixture below 8 bits.
- #15 (metrics dashboard) can put its pages under `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.

## Blockers / owner-todo
- #50: license ruling for test/build-only tools.
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner's fire-drill steps.
