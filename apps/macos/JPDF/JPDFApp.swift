// jpdf macOS app shell. Spec: specs/ui-macos/app-skeleton.md. No PDF logic here
// (CONSTITUTION 6): the engine will run in the JPDFHelper XPC service.
import SwiftUI

/// Why every open command is unavailable until the reader exists (M4).
let openUnavailableReason = "Opening files is not available yet"

@main
struct JPDFApp: App {
    var body: some Scene {
        // W-01/W-02: a single fixed-size start window (specs/ui/wireframes-macos.md).
        Window("jpdf", id: "start") {
            StartView()
        }
        .windowResizability(.contentSize)
        .commands {
            // File > Open… replaces New (there is nothing to create yet). Menu items use the
            // system's disabled state; rule 5's inert style is for controls inside views.
            CommandGroup(replacing: .newItem) {
                Button("Open…") {}
                    .keyboardShortcut("o")
                    .disabled(true)
            }
        }
    }
}
