import Foundation
import IrohaSwift

/// Minimal Connect session harness for local testing.
@main
struct ConnectMinimalApp {
    static func main() async {
        guard NoritoNativeBridge.shared.isAvailable else {
            print("Norito bridge unavailable; bundle NoritoBridge.xcframework before running.")
            return
        }

        // Replace with real endpoint/keys for live tests.
        let baseURL = URL(string: "https://torii.example")!
        let appPublicKey = Data(repeating: 0x01, count: 32)
        let sid = ConnectSid.generate(chainId: "sora", appPublicKey: appPublicKey, nonce16: Data(repeating: 0x02, count: 16))
        let keys = ConnectDirectionKeys(appToWallet: Data(repeating: 0xAA, count: 32),
                                        walletToApp: Data(repeating: 0xBB, count: 32))

        let diagnosticsRoot = ConnectSessionDiagnostics.defaultRootDirectory()
        let session = ConnectSession(baseURL: baseURL,
                                     sessionID: sid.rawBytes,
                                     flowControl: ConnectFlowControlWindow(defaultTokens: 8),
                                     diagnosticsRoot: diagnosticsRoot)
        session.setDirectionKeys(keys)

        print("Starting Connect session for sid=\(sid.base64URLEncoded)")
        do {
            for try await event in session.eventStream() {
                switch event {
                case .ciphertext(let frame):
                    print("ciphertext seq=\(frame.sequence) dir=\(frame.direction)")
                case .control(let control):
                    print("control=\(control)")
                }
            }
        } catch {
            print("Connect session terminated: \(error)")
        }

        // Export evidence bundle for inspection.
        let recorder = ConnectSessionDiagnostics(sessionID: sid.rawBytes,
                                                 configuration: .init(rootDirectory: diagnosticsRoot))
        let bundleDir = diagnosticsRoot.appendingPathComponent("connect-minimal-bundle", isDirectory: true)
        do {
            let manifest = try recorder.exportJournalBundle(to: bundleDir)
            try recorder.exportQueueMetrics(to: bundleDir.appendingPathComponent("metrics.ndjson"))
            print("Exported bundle to \(bundleDir.path) (files: \(manifest.files))")
        } catch {
            print("Failed to export bundle: \(error)")
        }
    }
}
