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

    /// Read the actual receipt-bound cursor, independent of later global ledger activity.
    public func loadFinalityProgress(receipt: Data) throws -> KagemushaWalletLoadProofProgressV1 {
        try .init(setup(.init(selector: 31, first: receipt)))
    }

    /// Verify and prove exactly one next original block for this receipt's own durable cursor.
    public func ingestLoadFinality(receipt: Data, original: Data) throws -> KagemushaWalletLoadProofProgressV1 {
        try .init(setup(.init(selector: 32, first: receipt, second: original)))
    }

    /// Produce the complete recursive proof from the selected Native history and actual originals.
    /// Receipt data, HTTP status and ordinary finality alone cannot credit the wallet.
    public func proveLoadFinality(receipt: Data, eventProof: Data) throws -> Data {
        let result = try setup(.init(selector: 28, first: receipt, second: eventProof))
        guard result.status == 41 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
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

    /// Recover Native's durable confirmation of the exact retained signed Activate transaction.
    public func confirmLedgerActivation(signedWire: Data) throws -> KagemushaWalletActivationConfirmationV1 {
        try .init(setup(.init(selector: 35, first: signedWire)))
    }

    /// Verify one next original block for this Activate's independent Native history.
    public func ingestActivationFinality(signedWire: Data, original: Data) throws -> KagemushaWalletActivationFinalityV1 {
        try .init(setup(.init(selector: 36, first: signedWire, second: original)))
    }

    /// Read the exact retained transaction's progress, independently of the main ledger cursor.
    public func activationFinalityProgress(signedWire: Data) throws -> KagemushaWalletActivationFinalityV1 {
        try .init(setup(.init(selector: 37, first: signedWire)))
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

/// Native-selected per-request proving progress. Receipt height is a DATA locator until completion.
public struct KagemushaWalletLoadProofProgressV1: Sendable {
    public let receiptHeight: UInt64
    public let verifiedHeight: UInt64
    init(_ result: KagemushaWalletCallV1) throws {
        guard result.status == 43, result.bytes.count == 8,
            result.sequenceLow > 1, result.sequenceHigh == 0, result.detail == 0
        else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        let verified = result.bytes.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
        guard verified <= result.sequenceLow else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        receiptHeight = result.sequenceLow; verifiedHeight = verified
    }
}

/// Native-verified successful inclusion of the account's exact retained signed Activate.
public struct KagemushaWalletActivationConfirmationV1: Sendable {
    public let height: UInt64
    public let blockHash: Data
    init(_ result: KagemushaWalletCallV1) throws {
        guard result.status == 44 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        height = result.sequenceLow
        blockHash = result.bytes
    }
}

/// Native-owned Activate history. Only a confirmation establishes activation.
public struct KagemushaWalletActivationFinalityV1: Sendable {
    public let confirmation: KagemushaWalletActivationConfirmationV1?
    public let verifiedHeight: UInt64?
    public let blockHash: Data?
    init(_ result: KagemushaWalletCallV1) throws {
        switch result.status {
        case 44:
            confirmation = try .init(result)
            verifiedHeight = result.sequenceLow
            blockHash = result.bytes
        case 45:
            confirmation = nil
            verifiedHeight = result.sequenceLow
            blockHash = result.bytes
        case 46:
            confirmation = nil
            verifiedHeight = nil
            blockHash = nil
        default:
            throw KagemushaWalletErrorV1.invalidNativeOutput
        }
    }
}
