# macOS app skeleton and E2E harness

Area: ui-macos
ISO 32000-2 clauses: none (application shell; no PDF logic)
Journeys: none yet. The skeleton draws the W-02 start window that J-RD-01…J-RD-03 begin on;
those journeys belong to M4 and get their `e2e.macos.*` tests then. The harness ID is
`e2e.macos.app_start_empty` (docs/adr/0003-macos-project-and-e2e-ids.md; same rule as ADR
0002 item 2).
Status: implemented

## Behavior
`apps/macos/JPDF.xcodeproj` (ADR 0003) has three targets:
- **JPDF** → `jpdf.app`, a SwiftUI app (Swift 6 language mode, macOS 14+), bundle ID
  `io.github.jacobdam.jpdf`. It uses a `Window` scene, not yet `DocumentGroup`: a document
  group brings a working Open… and New, which would contradict "unavailable" until the
  reader exists. The document scene comes with the reader (M4).
- **JPDFHelper** → `JPDFHelper.xpc`, an XPC service embedded in
  `jpdf.app/Contents/XPCServices/`. It will host jpdf-engine through the UniFFI bindings
  (PLAN §2; a later issue). Today it answers one call, `ping`, with a fixed string; the app
  doesn't call it yet. It contains no PDF logic (CONSTITUTION 6).
- **JPDFUITests**: the XCUITest bundle.

1. **Start window (W-02, specs/ui/wireframes-macos.md §W-01, §W-02).** One `Window` scene
   titled "jpdf", fixed at 720 × 480 points (`.windowResizability(.contentSize)`), shown at
   launch. Top to bottom, 20 pt margins:
   - "jpdf" in `.title2`, an accessibility header;
   - 20 below, the drop target: a rounded rectangle (radius 12, 1 pt dashed
     `separatorColor` stroke, up to 400 wide, at least 160 high) with "Drop a PDF here to
     open it" and the `Open…` button (`.borderedProminent`, the default button, so Return
     activates it);
   - 20 below, "Files you open appear here." in `.secondary` (W-02: no recent files).
   The elements carry accessibility identifiers `start.heading`, `start.drop.label`,
   `start.open` and `start.recent.empty` (the title bar also shows the text "jpdf").
   The W-01 Create buttons, the Recent list and drop highlighting come with the features
   that fill them. The drop target has no drop handler, so the system refuses drops.
2. **`Open…` is unavailable** until the reader exists (M4). Per wireframes-macos.md §0
   rule 5 it is not `.disabled(true)`: it stays enabled (Full Keyboard Access reaches it),
   drawn at 50 % opacity, with help tag and accessibility hint "Opening files is not
   available yet" (the web skeleton's wording) and accessibility value "unavailable".
   Activating it (click, Space, or Return as the default button) shows that sentence in a
   popover and does nothing else: no open panel, sheet or window.
3. **Menus.** File > `Open…` (⌘O, specs/ui/shortcuts.md) replaces the File menu's New item
   and is a disabled menu item (menus use the system's disabled state; rule 5 is about
   controls inside views). The other menus are the system defaults.
4. **Sandbox and hardened runtime.** Both the app and the helper are built with the hardened
   runtime and signed ad hoc (`-`; signing with a Developer ID comes with distribution).
   Entitlements are the committed files in `apps/macos/Config/`:
   - app: `com.apple.security.app-sandbox`, `com.apple.security.files.user-selected.read-write`
     (files only through the open and save panels);
   - helper: `com.apple.security.app-sandbox` only (no file access at all, PLAN §2).
   Release builds don't inject base entitlements (`CODE_SIGN_INJECT_BASE_ENTITLEMENTS = NO`),
   so the shipped entitlements are exactly these files with no `get-task-allow`. Debug test
   builds get Xcode's testing exceptions added automatically; they are never shipped.
5. **`cargo xtask ci macos --out DIR`** (CONTRACT §2), run from the repository root:
   1. deletes `DIR/e2e.json` and `DIR/macos.xcresult` from any earlier run;
   2. checks that `apps/macos/JPDF.xcodeproj` and `xcodebuild` exist;
   3. `xcodebuild … -scheme JPDF -destination platform=macOS,arch=<arch> -resultBundlePath
      DIR/macos.xcresult test` (Debug; `<arch>` is the architecture xtask was built for, so exactly one
      "My Mac" matches), with derived data in `target/xtask-macos/` (not in
      `DIR`, which CI uploads);
   4. reads the bundle with `xcrun xcresulttool get test-results tests` and maps every
      "Test Case" node to a test ID: a method `test_<id>()` or `test_<id>__<detail>()`
      reports `e2e.macos.<id>`; an ID takes the worst result of its tests (fail > skip >
      pass; "Expected Failure" is fail). A test case that names no ID (a misnamed test, or
      the "System Failures" case Xcode adds when the runner can't start) is an error;
   5. builds Release and checks, with `codesign --verify --deep --strict`, `codesign -dv`
      and `codesign -d --entitlements - --xml` (converted by `plutil -convert json`), that
      the signatures are intact and that `jpdf.app` and its embedded
      `JPDFHelper.xpc` have the hardened runtime flag and exactly the entitlements of
      Behavior 4, each `true`; this runs even if the tests failed;
   6. always writes `DIR/e2e.json` (empty results if no tests ran). If step 3 failed, step 4
      found an error or step 5 failed, every `pass` is written as `fail`: such a run may not
      have run every test, or its tests ran against a build that doesn't ship as checked
      (#76);
   7. exits 0 only if every step passed, at least one ID was reported and every ID is
      `pass`. All errors are printed; the first is the summary line.

## Persistence
None. The skeleton stores nothing (no defaults, no files, no recent list).

## Undo
Not applicable (no document, no editing).

## Errors
- xtask: a missing project or tool, a failing step, an unreadable result bundle, a damaged
  or oversized tool output (> 16 MiB, non-UTF-8, invalid JSON, nesting over 128) or a
  failed signature check prints `xtask: macos: <reason>` and exits 1. Nothing is ever read
  as a pass by default.
- XCUITest can't run when macOS Automation Mode needs a password (a local Mac that hasn't
  run `automationmodetool enable-automationmode-without-authentication`). xtask then
  reports the "System Failures" case as an error and an empty result set; the Release
  signature check still runs.

## Limits
No user input is processed by the app. xtask reads at most 16 MiB (+1 byte to detect
overflow) from each output stream of each tool, then closes it (and kills the tool on a
stdout overflow before waiting for stderr), and
accepts JSON nested at most 128 deep; IDs must match `e2e.macos.[a-z0-9_]+`, so
`e2e.json` needs no escaping.

## Security
- App Sandbox and hardened runtime on both processes, checked on the shipped configuration
  on every `ci macos` run (Behavior 4, 5.6). The helper has no file entitlement; the app
  only the user-selected one.
- The helper accepts XPC connections without checking the peer: an XPC service inside the
  app bundle is reachable only from that app (launchd scopes it to the bundle). When the
  engine moves in, every message is validated by the core (PLAN §2, CONSTITUTION 5).
- No network, no JavaScript, no web views. The app accepts no drops.
- xtask runs only Apple tools from Xcode and the system (`xcodebuild`, `xcrun`,
  `codesign`, `plutil`); there are no third-party Swift packages. Tool output is read up
  to 16 MiB per pipe (stdout here, stderr on a thread); input for a tool (the entitlements
  plist for `plutil`) is written on its own thread at the same time, so a tool that writes
  before it has read all its input can't deadlock the pipes. A failed write (the tool closed
  its stdin first) is an error, reported after a failed exit (#83). Input that fits the pipe
  buffer may be accepted even if the tool never reads it; only larger input is certain to
  fail. Not covered: a tool that exits but leaves a child holding its stdin open without
  reading would block `capture` (the Apple tools used here don't).

## Accessibility
The heading has the header trait; `Open…` stays in the keyboard order with its reason as
accessibility hint and "unavailable" as value (rule 5); system fonts, semantic colors and
system controls only, so Increase Contrast, Dark Mode and text size work as the system
provides. The popover is the system's (Reduce Motion applies). Tested: `Open…` is enabled
and its accessibility value is "unavailable". Not tested: the hint and help tag (XCUITest
has no API to read them), the 50 % dimming and activation by Space. An automated XCUITest
accessibility audit (`performAccessibilityAudit`, macOS 14+) comes with the reader (M4).

## Platforms
macOS only. The web counterpart is specs/ui-web/app-skeleton.md; both use the wording
"Drop a PDF here to open it", "Files you open appear here." and "Opening files is not
available yet". macOS uses `Open…` (HIG ellipsis, wireframes-macos.md §0 rule 2) where the
web says `Open file`.

## Performance
No budget yet; the first-page budget (J-RD-01) applies from M4. A `ci macos` run takes
about 2–4 minutes on a Mac mini (two builds plus the UI tests).

## Tests
XCUITest (`apps/macos/JPDFUITests/StartWindowTests.swift`), all reporting
`e2e.macos.app_start_empty`:
- `test_app_start_empty__elements`: one window titled "jpdf", 720 wide; the heading, drop
  text, `Open…` and the recent line each exist exactly once (by identifier) with the exact
  label, in that top-to-bottom order; `Open…` is enabled
  (inert, not disabled) with value "unavailable";
- `test_app_start_empty__open_is_inert`: clicking `Open…` shows the reason in a popover and
  opens no sheet or dialog; Escape closes it and only the one window exists; Return
  (default button) shows it again and still opens nothing (same checks);
- `test_app_start_empty__open_command`: File > `Open…` exists and is disabled. Not
  tested: that no New item remains (a single `Window` scene never offers one, so such a
  check couldn't fail) and the ⌘O shortcut.
Rust unit tests: `xtask/src/macos.rs` (test-name → ID mapping, ID validation, worst result
per ID, unmapped and unknown results are errors, passes downgraded on a failed `xcodebuild
test`, an unmapped result or a failed Release check (each alone), entitlement check incl. extra
`get-task-allow` and testing exceptions, hardened runtime flag parsing, tool capture: a
`/bin/sh` tool that writes 300 000 bytes before reading 300 000 bytes of input finishes
within 30 s, unread input is an error, a failed exit wins over it);
`xtask/src/json.rs` (values, escapes and surrogates, 27 malformed inputs, depth bound).
