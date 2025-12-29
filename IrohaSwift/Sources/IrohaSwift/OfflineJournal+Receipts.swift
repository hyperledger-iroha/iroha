import Foundation

public extension OfflineJournal {
    @discardableResult
    func appendPending(receipt: OfflineSpendReceipt, timestampMs: UInt64? = nil) throws -> OfflineJournalEntry {
        let payload = try receipt.noritoEncoded()
        return try appendPending(txId: receipt.txId, payload: payload, timestampMs: timestampMs)
    }

    func markCommitted(receipt: OfflineSpendReceipt, timestampMs: UInt64? = nil) throws {
        try markCommitted(txId: receipt.txId, timestampMs: timestampMs)
    }
}
