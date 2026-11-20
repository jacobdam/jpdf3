# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #46 (#42, journeys 5/5: signatures and platform conventions,
  specs/journeys/signatures-platform.md, J-SG-01…11, J-PC-01…07): reviewed; auto-merge on,
  waiting for judge-verdict.

## Last session
- Wrote the last journeys batch. The journeys backlog (#6–#9, #42) is complete: every
  RESEARCH.md row now names a journey, Phase 2 or out. Filled TRACE F-SC-2…F-SC-4 and R12, and
  the IA rows (Signatures panel, the Secure row, new dialogs, menus, preferences). W-21/W-22 now
  point to J-PC-01/J-PC-02.
- Filed discovery issues #47 (web CSP and macOS ATS for TSA/OCSP/CRL requests, needed before
  M9) and #48 (trust anchors beyond the user's own list). Added the new dialogs to #45's
  wireframe scope.
- Removed two stale worktrees from merged PRs (#43, #44).

## Next step
- #12 (web skeleton + Playwright) / #13 (macOS skeleton + XCUITest), which implement the W-05
  layout. Then #14, #17, #18, #20 and #22 (all `ready`). #45 (wireframes) is low priority.
- #21 still waits on #12, #13 and #19.

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
