# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #49 (#12, web app skeleton + Playwright harness in `cargo xtask ci linux`,
  specs/ui-web/app-skeleton.md, ADR 0002): reviewed; auto-merge on, waiting for
  judge-verdict. After merge, Pages should show the W-02 start screen.

## Last session
- Built `apps/web` (Vite 7, React 19, MUI 9, strict meta CSP with Trusted Types) and the
  Playwright harness; `e2e.json` now reports `e2e.web.app_start_empty` (a harness ID, not a
  journey ID, ADR 0002 item 2).
- License finding: CONSTITUTION 10 (no build-only exemption) excludes Vite 8, ESLint,
  Babel plugins and axe-core; required tools (Playwright, Vite, Rollup) also bundle
  CC-BY/BlueOak/CC0/0BSD code. Disclosed in ADR 0002, proposal
  ops/proposals/test-only-licenses.md, owner-todo #50.

## Next step
- #13 (macOS skeleton + XCUITest, the W-05 layout). Then #14, #17, #18, #20, #22 (`ready`).
  #15 (metrics dashboard) can now put its pages under `site/metrics/`.
- When the owner rules on #50: add axe-core to the harness (accessibility.md §5) or keep the
  hand-written checks.
- #21 still waits on #13 and #19. #45 (wireframes) is low priority.

## Blockers / owner-todo
- #50: license ruling for test/build-only tools.
- #21: the owner's rule fire-drill steps.
