# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #51 (#13, the macOS skeleton and `cargo xtask ci macos`; spec
  specs/ui-macos/app-skeleton.md, ADR 0003). Reviewed, with auto-merge on.
  - `project-macos` passed on the branch via `workflow_dispatch` (run 44820544986: Xcode 16.4,
    3/3 XCUITests, the sandbox and hardened runtime check passed). The last push only
    hardened xtask's `capture` and edited the spec; it was checked locally, not on the macOS
    runner.
  - After it merges, check that `project-macos` on main is green.

## Last session
- Built `apps/macos`:
  - a hand-written Xcode project with synchronized folders;
  - the W-02 start window (`Open…` inert, File > Open… disabled);
  - a sandbox-only XPC helper with `ping`.
- Added `cargo xtask ci macos`:
  - runs XCUITest and reads its results with xcresulttool through a strict JSON reader;
  - maps test names `test_<id>[__<detail>]` to IDs;
  - checks the Release codesign seal, hardened runtime and exact entitlements.
- Local XCUITest is blocked until the owner enables Automation Mode (#52); CI runs it.

## Next step
- `ready` issues: #14 (jpdf-traits), #17, #18, #20, #22. #15 (metrics dashboard) can put its
  pages under `site/metrics/`.
- #21 (fire drill) waits on #19; #13 is done once #51 merges.
- A later issue wires `bindings/jpdf-uniffi` into the helper (the app doesn't call it yet).
- #45 (wireframes) is low priority.

## Blockers / owner-todo
- #50: license ruling for test/build-only tools.
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner's fire-drill steps.
