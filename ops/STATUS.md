# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #49 (#12, web app skeleton + Playwright harness in `cargo xtask ci linux`,
  specs/ui-web/app-skeleton.md, ADR 0002): CI went red after the last push of the previous
  session. Fixed (CI attempt 1 of 3), re-reviewed, auto-merge on, waiting for judge-verdict.
  After merge, Pages should show the W-02 start screen.

## Last session
- Fixed PR #49's red check. A Playwright color assertion depended on timing: the disabled
  `Open file` button darkened on hover, a real UX bug. The button is now flat and keeps its
  look and its two-tone focus ring on hover, and tests wait for transitions to finish.
- Before that: built `apps/web` (Vite 7, React 19, MUI 9, strict CSP, Trusted Types) and the
  harness. License gap (CONSTITUTION 10 has no build-only exemption) is disclosed in ADR 0002,
  ops/proposals/test-only-licenses.md and owner-todo #50.

## Next step
- #13 (macOS skeleton + XCUITest, the W-05 layout). Then #14, #17, #18, #20, #22 (`ready`).
  #15 (metrics dashboard) can now put its pages under `site/metrics/`.
- When the owner rules on #50: add axe-core to the harness (accessibility.md §5) or keep the
  hand-written checks.
- #21 still waits on #13 and #19. #45 (wireframes) is low priority.

## Blockers / owner-todo
- #50: license ruling for test/build-only tools.
- #21: the owner's rule fire-drill steps.
