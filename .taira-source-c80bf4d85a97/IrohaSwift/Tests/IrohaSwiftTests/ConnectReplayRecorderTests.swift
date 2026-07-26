import Foundation
import XCTest

@testable import IrohaSwift

final class ConnectReplayRecorderTests: XCTestCase {
    func testRecordsCiphertextFramesAndUpdatesDiagnostics() throws {
        try requireBlake3()
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)

        let sessionID = Data(repeating: 0x11, count: 32)
        let recorder = ConnectReplayRecorder(sessionID: sessionID,
                                             diagnosticsRoot: tmp,
                                             fileManager: .default)
        let ciphertext = Data([0xAA, 0xBB, 0xCC])
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: .appToWallet,
                                 sequence: 1,
                                 kind: .ciphertext(.init(payload: ciphertext)))

        let snapshot = try recorder.record(frame: frame, timestampMs: 1_234)
        XCTAssertEqual(snapshot.appToWallet.depth, 1)
        XCTAssertEqual(snapshot.appToWallet.bytes, ciphertext.count)
        XCTAssertEqual(snapshot.appToWallet.oldestSequence, 1)
        XCTAssertEqual(snapshot.state, .healthy)

        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        let records = try journal.records(direction: .appToWallet, nowMs: 2_000)
        XCTAssertEqual(records.count, 1)
        XCTAssertEqual(records.first?.ciphertext, ciphertext)

        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tmp),
                                                    fileManager: .default)
        let metricsURL = tmp.appendingPathComponent("metrics.ndjson")
        try diagnostics.exportQueueMetrics(to: metricsURL)
        let lines = try String(contentsOf: metricsURL).split(separator: "\n")
        XCTAssertEqual(lines.count, 1)
        let sampleData = Data(lines[0].utf8)
        let sample = try JSONDecoder().decode(ConnectQueueMetricsSample.self, from: sampleData)
        XCTAssertEqual(sample.appToWalletDepth, 1)
        XCTAssertEqual(sample.walletToAppDepth, 0)
        XCTAssertEqual(sample.state, .healthy)
    }

    func testControlFramesDoNotChangeDepth() throws {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)

        let sessionID = Data(repeating: 0x22, count: 32)
        let recorder = ConnectReplayRecorder(sessionID: sessionID,
                                             diagnosticsRoot: tmp,
                                             fileManager: .default)
        let control = ConnectControl.close(ConnectClose(role: .app, code: 1000, reason: nil, retryable: false))
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: .walletToApp,
                                 sequence: 5,
                                 kind: .control(control))

        let snapshot = try recorder.record(frame: frame, timestampMs: 2_000)
        XCTAssertEqual(snapshot.appToWallet.depth, 0)
        XCTAssertEqual(snapshot.walletToApp.depth, 0)
        XCTAssertEqual(snapshot.state, .healthy)

        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tmp),
                                                    fileManager: .default)
        let metricsURL = tmp.appendingPathComponent("metrics-control.ndjson")
        try diagnostics.exportQueueMetrics(to: metricsURL)
        let lines = try String(contentsOf: metricsURL).split(separator: "\n")
        XCTAssertEqual(lines.count, 1)
        let sampleData = Data(lines[0].utf8)
        let sample = try JSONDecoder().decode(ConnectQueueMetricsSample.self, from: sampleData)
        XCTAssertEqual(sample.appToWalletDepth, 0)
        XCTAssertEqual(sample.walletToAppDepth, 0)
        XCTAssertEqual(sample.state, .healthy)
    }

    private func requireBlake3() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
    }
}
