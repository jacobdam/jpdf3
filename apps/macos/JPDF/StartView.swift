// W-02 start window, no recent files (specs/ui/wireframes-macos.md §W-01, §W-02).
import SwiftUI

struct StartView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("jpdf")
                .font(.title2)
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("start.heading")
            DropTarget()
                .padding(.top, 20)
            Text("Files you open appear here.")
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("start.recent.empty")
                .padding(.top, 20)
            Spacer(minLength: 0)
        }
        .padding(20)
        .frame(width: 720, height: 480, alignment: .topLeading)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

/// The dashed drop target with the default `Open…` button. Until the reader exists it accepts
/// no drops: SwiftUI refuses a drop on a view without a drop handler.
private struct DropTarget: View {
    var body: some View {
        VStack(spacing: 12) {
            Text("Drop a PDF here to open it")
                .accessibilityIdentifier("start.drop.label")
            InertButton(title: "Open…", reason: openUnavailableReason)
                .accessibilityIdentifier("start.open")
        }
        .frame(maxWidth: 400, minHeight: 160)
        .background {
            RoundedRectangle(cornerRadius: 12)
                .strokeBorder(
                    Color(nsColor: .separatorColor),
                    style: StrokeStyle(lineWidth: 1, dash: [6, 4])
                )
        }
    }
}

/// wireframes-macos.md §0 rule 5: an unavailable control stays enabled (so Full Keyboard
/// Access reaches it) but inert, drawn dimmed, with its reason as help tag and accessibility
/// hint and the accessibility value "unavailable". Activating it only shows the reason.
private struct InertButton: View {
    let title: String
    let reason: String
    @State private var showingReason = false

    var body: some View {
        Button(title) { showingReason = true }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.defaultAction)
            .opacity(0.5)
            .help(reason)
            .accessibilityValue("unavailable")
            .accessibilityHint(reason)
            .popover(isPresented: $showingReason, arrowEdge: .bottom) {
                Text(reason)
                    .padding(12)
            }
    }
}
