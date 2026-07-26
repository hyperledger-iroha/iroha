import XCTest
@testable import IrohaSwift

final class ConnectQueueDiagnosticsTests: XCTestCase {
    func testSnapshotDefaultsWhenMissing() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

        let sessionID = Data([0x01, 0x02, 0x03, 0x04])
        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tempDir))
        let snapshot = try diagnostics.snapshot()
        XCTAssertEqual(snapshot.state, .disabled)
        XCTAssertEqual(snapshot.sessionIDBase64, sessionID.base64URLEncodedString())
        XCTAssertEqual(snapshot.appToWallet.depth, 0)
        XCTAssertEqual(snapshot.walletToApp.bytes, 0)
    }

    func testTrackerPersistsSnapshotAndMetrics() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

        let sessionID = Data([0xaa, 0xbb, 0xcc, 0xdd, 0xee])
        let tracker = ConnectQueueStateTracker(sessionID: sessionID,
                                               configuration: .init(rootDirectory: tempDir))
        try tracker.updateSnapshot { snapshot in
            snapshot.state = .throttled
            snapshot.reason = "disk_watermark"
            snapshot.appToWallet.depth = 5
            snapshot.appToWallet.bytes = 1024
        }
        try tracker.recordMetric(ConnectQueueMetricsSample(timestampMs: 1234,
                                                           state: .throttled,
                                                           appToWalletDepth: 5,
                                                           walletToAppDepth: 0,
                                                           reason: "disk_watermark"))

        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tempDir))
        let snapshot = try diagnostics.snapshot()
        XCTAssertEqual(snapshot.state, .throttled)
        XCTAssertEqual(snapshot.reason, "disk_watermark")
        XCTAssertEqual(snapshot.appToWallet.depth, 5)
        XCTAssertEqual(snapshot.appToWallet.bytes, 1024)

        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tempDir),
                                          fileManager: .default)
        let metricsURL = storage.metricsURL
        XCTAssertTrue(FileManager.default.fileExists(atPath: metricsURL.path))
        let metricsContents = try String(contentsOf: metricsURL)
        XCTAssertTrue(metricsContents.contains("\"state\":\"throttled\""))
    }

    func testExportJournalBundleCopiesArtifacts() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

        let sessionID = Data([0xff, 0xee, 0xdd, 0xcc])
        let tracker = ConnectQueueStateTracker(sessionID: sessionID,
                                               configuration: .init(rootDirectory: tempDir))
        try tracker.updateSnapshot { snapshot in
            snapshot.state = .healthy
        }
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tempDir),
                                          fileManager: .default)
        try FileManager.default.createDirectory(at: storage.sessionDirectory,
                                                withIntermediateDirectories: true)
        let appQueueURL = storage.appQueueURL
        let walletQueueURL = storage.walletQueueURL
        try "app-data".data(using: .utf8)!.write(to: appQueueURL)
        try "wallet-data".data(using: .utf8)!.write(to: walletQueueURL)

        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tempDir))
        let targetDir = tempDir.appendingPathComponent("export", isDirectory: true)
        let manifest = try diagnostics.exportJournalBundle(to: targetDir)
        XCTAssertEqual(manifest.snapshot.state, .healthy)
        XCTAssertTrue(FileManager.default.fileExists(atPath: targetDir
            .appendingPathComponent("app_to_wallet.queue").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: targetDir
            .appendingPathComponent("wallet_to_app.queue").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: targetDir
            .appendingPathComponent("manifest.json").path))
    }
}

private extension Data {
    func base64URLEncodedString() -> String {
        let base64 = self.base64EncodedString()
        return base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
