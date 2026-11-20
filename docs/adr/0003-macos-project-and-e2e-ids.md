# 3. macOS project format, signing and E2E test IDs

Status: accepted (#13)

## Context
`cargo xtask ci macos --out DIR` must build the SwiftUI app with an XPC helper, run real
XCUITest tests and write `DIR/e2e.json` with `e2e.macos.*` journey IDs (CONTRACT §2). An
app bundle with an embedded XPC service, entitlements and a UI-test target needs an Xcode
project: Swift packages can't produce app bundles or UI-test bundles. XcodeGen and Tuist
are third-party tools outside dependency admission. The CI runner (`macos-15`) has Xcode
16 or later and no signing identity.

## Decision
1. **A committed, hand-written `JPDF.xcodeproj`** using Xcode 16 synchronized folders
   (`PBXFileSystemSynchronizedRootGroup`, `objectVersion = 77`). Source files are picked up
   from `JPDF/`, `JPDFHelper/` and `JPDFUITests/` without being listed, so adding a Swift
   file doesn't change `project.pbxproj`; the file only changes when targets or build
   settings change, which keeps diffs small and reviewable. Object IDs are readable
   sequential values. Entitlements and the helper's `Info.plist` live in `Config/`, outside
   the synchronized folders. The shared scheme `JPDF` (in `xcshareddata`) builds the app and
   runs `JPDFUITests`. No build setting that needs Xcode 26 is used.
2. **Ad-hoc signing** (`CODE_SIGN_IDENTITY = "-"`, manual style, no team) everywhere, so
   CI and any Mac build without credentials. Hardened runtime is on for the app and the
   helper and off for the UI-test bundle: the test runner would otherwise refuse to load
   the ad-hoc signed bundle (library validation, "different Team IDs"). Release builds set
   `CODE_SIGN_INJECT_BASE_ENTITLEMENTS = NO`, so their entitlements are exactly the
   committed files. Developer ID signing and notarization come with distribution.
3. **The shipped configuration is checked by xtask**, not trusted: after the tests,
   `ci macos` builds Release and verifies the hardened runtime flag and the exact
   entitlement set of `jpdf.app` and `JPDFHelper.xpc`. Debug test builds can't be checked
   this way because Xcode adds testing exceptions (read access to `/`, testmanagerd
   lookups) to them.
4. **Test IDs come from XCTest method names**: `test_<id>()` or `test_<id>__<detail>()`
   reports `e2e.macos.<id>`. Several tests may report the same ID; the worst result wins
   (fail > skip > pass), as in ADR 0002 item 1. Swift method names can't carry dots, and
   XCTest has no tags, so the name is the only stable place. Every test case must map to an
   ID; anything else (including Xcode's "System Failures" case) fails the run.
5. **Results come from the result bundle** through `xcrun xcresulttool get test-results
   tests` (Xcode 16+), not from console output. xtask parses that JSON with its own small
   strict reader (`xtask/src/json.rs`), because xtask has no dependencies (ADR 0002 Context).
6. **Harness ID** `e2e.macos.app_start_empty` follows ADR 0002 item 2: no journey lists it,
   so it never changes a grade; journeys add their own IDs in M4.

## Consequences
- Adding targets or build settings means editing `project.pbxproj` by hand (or in Xcode,
  then reviewing the diff for unrelated churn such as reordered sections).
- Local runs need macOS Automation Mode without a password prompt
  (`sudo automationmodetool enable-automationmode-without-authentication`, once per Mac);
  GitHub's macOS runners already have it. Until then local runs report the System Failures
  error.
