// JPDFHelper XPC service (PLAN §2): the process that will host jpdf-engine through the UniFFI
// bindings. It is sandboxed with no file entitlements: it will receive document bytes from the
// app, never paths. It holds no PDF logic in Swift (CONSTITUTION 6).
// Spec: specs/ui-macos/app-skeleton.md.
import Foundation

/// The helper's XPC interface. Until the engine is wired in, it only answers `ping`.
@objc protocol JPDFHelperProtocol {
    func ping(reply: @escaping @Sendable (String) -> Void)
}

final class HelperService: NSObject, JPDFHelperProtocol {
    func ping(reply: @escaping @Sendable (String) -> Void) {
        reply("jpdf-helper")
    }
}

final class HelperDelegate: NSObject, NSXPCListenerDelegate {
    func listener(
        _ listener: NSXPCListener,
        shouldAcceptNewConnection connection: NSXPCConnection
    ) -> Bool {
        // An XPC service inside the app bundle is reachable only from that app.
        connection.exportedInterface = NSXPCInterface(with: JPDFHelperProtocol.self)
        connection.exportedObject = HelperService()
        connection.resume()
        return true
    }
}

let delegate = HelperDelegate()
let listener = NSXPCListener.service()
listener.delegate = delegate
listener.resume()
