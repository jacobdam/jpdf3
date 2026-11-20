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

        // Identifiers, not labels: the title bar also shows the text "jpdf".
        let heading = window.staticTexts.matching(identifier: "start.heading")
        let drop = window.staticTexts.matching(identifier: "start.drop.label")
        let openQuery = window.buttons.matching(identifier: "start.open")
        let recent = window.staticTexts.matching(identifier: "start.recent.empty")
        let expected: [(XCUIElementQuery, String)] = [
            (heading, "jpdf"),
            (drop, "Drop a PDF here to open it"),
            (openQuery, "Open…"),
            (recent, "Files you open appear here."),
        ]
        for (query, label) in expected {
            XCTAssertEqual(query.count, 1, label)
            XCTAssertEqual(query.firstMatch.label, label)
        }
        let open = openQuery.firstMatch
        // Top-to-bottom order of W-02.
        XCTAssertLessThan(heading.firstMatch.frame.minY, drop.firstMatch.frame.minY)
        XCTAssertLessThan(drop.firstMatch.frame.minY, open.frame.minY)
        XCTAssertLessThan(open.frame.minY, recent.firstMatch.frame.minY)

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
        window.buttons.matching(identifier: "start.open").firstMatch.click()
        let shown = app.popovers.staticTexts[reason]
        XCTAssertTrue(shown.waitForExistence(timeout: 5), "reason popover")
        // Nothing else happened: no open panel, no sheet, no new window.
        XCTAssertEqual(app.sheets.count, 0)
        XCTAssertEqual(app.dialogs.count, 0)
        app.typeKey(.escape, modifierFlags: [])
        XCTAssertTrue(shown.waitForNonExistence(timeout: 5), "popover closes")
        XCTAssertEqual(app.windows.count, 1, "no second window or panel")

        // Return triggers the default button, which is inert too.
        window.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(shown.waitForExistence(timeout: 5), "reason popover on Return")
        XCTAssertEqual(app.sheets.count, 0)
        XCTAssertEqual(app.dialogs.count, 0)
        app.typeKey(.escape, modifierFlags: [])
        XCTAssertTrue(shown.waitForNonExistence(timeout: 5), "popover closes")
        XCTAssertEqual(app.windows.count, 1, "no second window or panel")
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
        app.typeKey(.escape, modifierFlags: [])
    }
}
