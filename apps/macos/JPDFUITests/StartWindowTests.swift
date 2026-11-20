// XCUITest E2E for the start window (specs/ui-macos/app-skeleton.md, Tests).
// Test ID mapping (docs/adr/0003-macos-project-and-e2e-ids.md): a test method named
// `test_<id>` or `test_<id>__<detail>` reports journey test ID `e2e.macos.<id>`.
import XCTest

final class StartWindowTests: XCTestCase {
    private let reason = "Opening files is not available yet"

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func test_app_start_empty__elements() throws {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }

        let window = app.windows["jpdf"]
        XCTAssertTrue(window.waitForExistence(timeout: 20), "start window")
        XCTAssertEqual(app.windows.count, 1)
        XCTAssertEqual(window.frame.size.width, 720, accuracy: 1)

        let heading = window.staticTexts["jpdf"]
        let drop = window.staticTexts["Drop a PDF here to open it"]
        let open = window.buttons["Open…"]
        let recent = window.staticTexts["Files you open appear here."]
        for element in [heading, drop, open, recent] {
            XCTAssertTrue(element.exists, "\(element)")
        }
        // Top-to-bottom order of W-02.
        XCTAssertLessThan(heading.frame.minY, drop.frame.minY)
        XCTAssertLessThan(drop.frame.minY, open.frame.minY)
        XCTAssertLessThan(open.frame.minY, recent.frame.minY)

        // Rule 5: inert, not disabled, so keyboard navigation still reaches it.
        XCTAssertTrue(open.isEnabled)
        XCTAssertEqual(open.value as? String, "unavailable")
    }

    @MainActor
    func test_app_start_empty__open_is_inert() throws {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }

        let window = app.windows["jpdf"]
        XCTAssertTrue(window.waitForExistence(timeout: 20), "start window")
        window.buttons["Open…"].click()
        let shown = app.popovers.staticTexts[reason]
        XCTAssertTrue(shown.waitForExistence(timeout: 5), "reason popover")
        // Nothing else happened: no open panel, no sheet, no new window.
        XCTAssertEqual(app.sheets.count, 0)
        XCTAssertEqual(app.dialogs.count, 0)
        app.typeKey(.escape, modifierFlags: [])
        XCTAssertTrue(shown.waitForNonExistence(timeout: 5), "popover closes")

        // Return triggers the default button, which is inert too.
        window.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(shown.waitForExistence(timeout: 5), "reason popover on Return")
        XCTAssertEqual(app.sheets.count, 0)
        XCTAssertEqual(app.dialogs.count, 0)
    }

    @MainActor
    func test_app_start_empty__open_command() throws {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }

        XCTAssertTrue(app.windows["jpdf"].waitForExistence(timeout: 20), "start window")
        let file = app.menuBars.menuBarItems["File"]
        file.click()
        let open = file.menuItems["Open…"]
        XCTAssertTrue(open.waitForExistence(timeout: 5), "File > Open…")
        XCTAssertFalse(open.isEnabled, "nothing can be opened yet")
        XCTAssertFalse(file.menuItems["New Window"].exists, "New is replaced")
        app.typeKey(.escape, modifierFlags: [])
    }
}
