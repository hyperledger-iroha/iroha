import Foundation

/// Helper that captures Connect frames into the on-disk journal/metrics used for
/// replay fixtures and CLI inspection.
public final class ConnectReplayRecorder {
    private let sessionID: Data
    private let diagnosticsRoot: URL
    private let fileManager: FileManager
    private let journal: ConnectQueueJournal
    private let tracker: ConnectQueueStateTracker

    public init(sessionID: Data,
                diagnosticsRoot: URL = ConnectSessionDiagnostics.defaultRootDirectory(),
                fileManager: FileManager = .default) {
        let journalConfig = ConnectQueueJournal.Configuration(rootDirectory: diagnosticsRoot)
        let diagnosticsConfig = ConnectSessionDiagnostics.Configuration(rootDirectory: diagnosticsRoot)
        self.sessionID = sessionID
        self.diagnosticsRoot = diagnosticsRoot
        self.fileManager = fileManager
        self.journal = ConnectQueueJournal(sessionID: sessionID,
                                           configuration: journalConfig,
                                           fileManager: fileManager)
        self.tracker = ConnectQueueStateTracker(sessionID: sessionID,
                                                configuration: diagnosticsConfig,
                                                fileManager: fileManager)
    }

    /// Persist a received or sent frame to the journal and update queue metrics.
    @discardableResult
    public func record(frame: ConnectFrame,
                       timestampMs: UInt64 = ConnectSessionDiagnostics.timestampNow()) throws -> ConnectQueueSnapshot {
        switch frame.kind {
        case .ciphertext(let payload):
            try journal.append(direction: frame.direction,
                               sequence: frame.sequence,
                               ciphertext: payload.payload,
                               receivedAtMs: timestampMs)
            return try updateSnapshot(direction: frame.direction,
                                      payloadBytes: payload.payload.count,
                                      sequence: frame.sequence,
                                      timestampMs: timestampMs)
        case .control:
            return try updateSnapshot(direction: frame.direction,
                                      payloadBytes: 0,
                                      sequence: frame.sequence,
                                      timestampMs: timestampMs,
                                      controlOnly: true)
        }
    }

    /// Export the collected snapshot/metrics/queues into an evidence bundle.
    @discardableResult
    public func exportBundle(to targetDirectory: URL) throws -> ConnectQueueEvidenceManifest {
        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: diagnosticsRoot),
                                                    fileManager: fileManager)
        return try diagnostics.exportJournalBundle(to: targetDirectory)
    }

    private func updateSnapshot(direction: ConnectDirection,
                                payloadBytes: Int,
                                sequence: UInt64,
                                timestampMs: UInt64,
                                controlOnly: Bool = false) throws -> ConnectQueueSnapshot {
        var snapshotCopy: ConnectQueueSnapshot?
        try tracker.updateSnapshot { snapshot in
            snapshot.state = .healthy
            if !controlOnly || payloadBytes > 0 {
                var stats = direction == .appToWallet ? snapshot.appToWallet : snapshot.walletToApp
                stats.depth += 1
                stats.bytes += payloadBytes
                if stats.oldestSequence == nil { stats.oldestSequence = sequence }
                stats.newestSequence = sequence
                if stats.oldestTimestampMs == nil { stats.oldestTimestampMs = timestampMs }
                stats.newestTimestampMs = timestampMs
                if direction == .appToWallet {
                    snapshot.appToWallet = stats
                } else {
                    snapshot.walletToApp = stats
                }
            }
            snapshotCopy = snapshot
        }
        guard let snapshot = snapshotCopy else {
            throw ConnectQueueError.corrupted
        }

        let sample = ConnectQueueMetricsSample(timestampMs: timestampMs,
                                               state: snapshot.state,
                                               appToWalletDepth: snapshot.appToWallet.depth,
                                               walletToAppDepth: snapshot.walletToApp.depth,
                                               reason: snapshot.reason)
        try tracker.recordMetric(sample)
        return snapshot
    }
}
