import Foundation

/// Scheduling metadata from one exact canonical, signature-verified instruction transaction.
/// This does not prove submission, finality or execution.
public struct InstructionTransactionInspectionV1: Sendable {
    public let creationTimeMs: UInt64
    public let timeToLiveMs: UInt64?
    public let transactionHash: Data
    public let signerPublicKey: Data
}

extension InstructionTransactionV1 {
    /// Inspect a retained signed envelope against its exact expected instruction sequence.
    /// Uses the SDK's canonical transaction decoder and signature verifier; it creates no wire.
    public static func inspectSignedWire(
        _ signedWire: Data,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
        expectedFrames: [TransactionInstructionFrame]
    ) throws -> InstructionTransactionInspectionV1 {
        guard !expectedFrames.isEmpty else { throw ExecutableBatchInputError.emptyBatch }
        let inspected = try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
            signedWire, context: "retained instruction transaction")
        guard inspected.payload.networkId == expectedNetworkId else {
            throw ToriiClientError.invalidPayload("retained instruction transaction changed the expected network")
        }
        try ToriiCanonicalTransactionDraft.requireAuthority(inspected.payload, equals: expectedAuthority,
            context: "retained instruction transaction")
        var instructions = CompactNoritoWriter()
        instructions.writeUInt64LE(UInt64(expectedFrames.count))
        for frame in expectedFrames {
            instructions.writeField(try frame.compactInstructionBoxPayload(expectedNetworkId: expectedNetworkId))
        }
        var executable = CompactNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        guard executable.data == inspected.payload.executable else {
            throw ToriiClientError.invalidPayload("retained instruction transaction changed the expected instructions")
        }
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(inspected.transactionPayload)
        return InstructionTransactionInspectionV1(creationTimeMs: inspected.payload.creationTimeMs,
            timeToLiveMs: inspected.payload.timeToLiveMs, transactionHash: IrohaHash.hash(entrypoint.data),
            signerPublicKey: inspected.signerPublicKey)
    }
}
