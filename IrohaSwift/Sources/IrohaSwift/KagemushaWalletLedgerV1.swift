import Foundation

/// Closed original Native transports accepted by the current ledger instruction.
public enum KagemushaWalletLedgerTransportV1: UInt64, Sendable {
    case activate = 1, unload = 2, closeLoads = 3
}

extension KagemushaWalletV1 {
    /// Freeze an exact next Load instruction under durable Native request custody.
    /// Retry the same request ID and amount after uncertain delivery.
    public func prepareLedgerLoad(requestId: Data, amount: KagemushaWalletUInt128V1) throws -> Data {
        let result = try setup(.init(selector: 27, identity: requestId, amount: amount))
        guard result.status == 40 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        return result.bytes
    }

    /// Canonical instruction framing only; ledger execution still validates every authority.
    public func ledgerInstruction(kind: KagemushaWalletLedgerTransportV1, original: Data) throws -> Data {
        let result = try setup(.init(selector: 29, token: kind.rawValue, first: original))
        guard result.status == 40 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        return result.bytes
    }
    /// Read this exact transaction and Unload claim's independently retained history.
    public func unloadFinalityProgress(transactionHash: Data, original: Data) throws -> KagemushaWalletLedgerProgressV1? {
        let result = try setup(.init(selector: 33, identity: transactionHash, first: original))
        if result.status == 34 { return nil }
        return try .init(result)
    }
    /// Verify one next original block for this Unload's own durable history cursor.
    public func ingestUnloadFinality(transactionHash: Data, original: Data, finality: Data) throws -> KagemushaWalletLedgerProgressV1 {
        try .init(setup(.init(selector: 34, identity: transactionHash, first: original, second: finality)))
    }

    /// Confirm exact successful input/output inclusion of this original Unload in its own Native-selected block.
    public func confirmLedgerUnload(transactionHash: Data, original: Data) throws -> KagemushaWalletUnloadConfirmationV1 {
        let result = try setup(.init(selector: 30, identity: transactionHash, first: original))
        guard result.status == 42 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        return .init(height: result.sequenceLow, blockHash: result.bytes)
    }

}


/// Native-verified successful execution of the exact submitted Unload claim.
public struct KagemushaWalletUnloadConfirmationV1: Sendable {
    public let height: UInt64
    public let blockHash: Data
}
