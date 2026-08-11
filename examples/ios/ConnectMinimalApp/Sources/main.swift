import Foundation
import IrohaSwift

/// Minimal Connect session harness for local testing.
@main
struct ConnectMinimalApp {
    static func main() async {
        // Replace with real endpoint/keys for live tests.
        let baseURL = URL(string: "https://torii.example")!
        let networkID = try! NetworkId(
            literal: "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        )
        let appPublicKey = Data(repeating: 0x01, count: 32)
        let nonce = Data(repeating: 0x02, count: 16)
        let sid = try! ConnectCrypto.deriveSessionID(
            networkID: networkID,
            appPublicKey: appPublicKey,
            nonce: nonce
        )
        let keys = ConnectDirectionKeys(appToWallet: Data(repeating: 0xAA, count: 32),
                                        walletToApp: Data(repeating: 0xBB, count: 32))

        let diagnosticsRoot = ConnectSessionDiagnostics.defaultRootDirectory()
        let request = try! ConnectClient.makeWebSocketRequest(
            baseURL: baseURL,
            sid: base64URL(sid),
            role: .app,
            token: "replace-with-token-app"
        )
        let client = ConnectClient(request: request)
        let diagnostics = ConnectSessionDiagnostics(sessionID: sid)
        let session = try! ConnectSession(
            networkID: networkID,
            appPublicKey: appPublicKey,
            nonce: nonce,
            relayToken: "replace-with-token-relay",
            client: client,
            diagnostics: diagnostics
        )
        session.setDirectionKeys(keys)

        print("Prepared launch-bound Connect session for sid=\(base64URL(sid))")

        // Export evidence bundle for inspection.
        let recorder = ConnectSessionDiagnostics(sessionID: sid,
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

    private static func base64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
