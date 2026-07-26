import Foundation

/// Recorded transaction plus metadata persisted by a `PendingTransactionQueue`.
public struct PendingTransactionRecord: Codable, Sendable {
    public let envelope: SignedTransactionEnvelope
    public let enqueuedAtMs: UInt64

    public init(envelope: SignedTransactionEnvelope, enqueuedAtMs: UInt64? = nil) {
        self.envelope = envelope
        self.enqueuedAtMs = enqueuedAtMs ?? PendingTransactionRecord.nowMs()
    }

    private static func nowMs() -> UInt64 {
        IrohaSDK.defaultCreationTimeMs()
    }
}

/// Queue abstraction used to persist signed transactions when Torii is unreachable.
public protocol PendingTransactionQueue: AnyObject {
    /// Append the provided envelope to the queue with the current timestamp.
    func enqueue(_ envelope: SignedTransactionEnvelope) throws

    /// Append an existing record (used when requeuing during replay failures).
    func enqueue(_ record: PendingTransactionRecord) throws

    /// Remove and return every queued record in insertion order.
    func drain() throws -> [PendingTransactionRecord]

    /// Count queued entries without mutating the queue.
    func size() throws -> Int

    /// Remove every queued entry without returning them.
    func clear() throws
}
