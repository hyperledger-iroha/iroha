import Foundation

/// Errors emitted by native escrow instruction helpers.
public enum NativeEscrowInstructionBuilderError: LocalizedError, Equatable {
    case invalidValue(field: String)
    case invalidEvidenceHash(index: Int)

    public var errorDescription: String? {
        switch self {
        case let .invalidValue(field):
            return "\(field) must be a non-empty string"
        case let .invalidEvidenceHash(index):
            return "evidenceHashes[\(index)] must be a non-empty string"
        }
    }
}

/// Native escrow permission token names.
public enum NativeEscrowPermissions {
    /// Permission allowing a court account or role to resolve disputed native escrows.
    public static let canResolveEscrowDispute = "CanResolveEscrowDispute"
}

private enum NativeEscrowInstructionPayloadBuilder {
    static func normalized(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw NativeEscrowInstructionBuilderError.invalidValue(field: field)
        }
        return trimmed
    }

    static func normalizedEvidence(_ values: [String]) throws -> [String] {
        try values.enumerated().map { index, value in
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw NativeEscrowInstructionBuilderError.invalidEvidenceHash(index: index)
            }
            return trimmed
        }
    }

    static func instruction(named name: String, payload: [String: Any]) throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([name: payload])
    }

    static func escrowOnlyInstruction(named name: String, escrowId: String) throws -> NoritoJSON {
        try instruction(named: name, payload: [
            "escrow_id": try normalized(escrowId, field: "escrowId"),
        ])
    }
}

/// Swift helpers for building native numeric asset escrow instruction payloads as Norito JSON.
public enum NativeEscrowInstructionBuilders {
    /// Build an `OpenAssetEscrow` instruction payload.
    public static func openAssetEscrow(escrowId: String,
                                       assetDefinition: String,
                                       amount: String,
                                       evidenceHashes: [String] = []) throws -> NoritoJSON {
        var payload: [String: Any] = [
            "escrow_id": try NativeEscrowInstructionPayloadBuilder.normalized(escrowId, field: "escrowId"),
            "asset_definition": try NativeEscrowInstructionPayloadBuilder.normalized(assetDefinition,
                                                                                     field: "assetDefinition"),
            "amount": try NativeEscrowInstructionPayloadBuilder.normalized(amount, field: "amount"),
        ]
        let evidence = try NativeEscrowInstructionPayloadBuilder.normalizedEvidence(evidenceHashes)
        if !evidence.isEmpty {
            payload["evidence_hashes"] = evidence
        }
        return try NativeEscrowInstructionPayloadBuilder.instruction(named: "OpenAssetEscrow", payload: payload)
    }

    /// Build an `AcceptAssetEscrow` instruction payload.
    public static func acceptAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionPayloadBuilder.escrowOnlyInstruction(named: "AcceptAssetEscrow",
                                                                       escrowId: escrowId)
    }

    /// Build a `MarkEscrowPaymentSent` instruction payload.
    public static func markEscrowPaymentSent(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionPayloadBuilder.escrowOnlyInstruction(named: "MarkEscrowPaymentSent",
                                                                       escrowId: escrowId)
    }

    /// Build a `ReleaseAssetEscrow` instruction payload.
    public static func releaseAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionPayloadBuilder.escrowOnlyInstruction(named: "ReleaseAssetEscrow",
                                                                       escrowId: escrowId)
    }

    /// Build a `CancelAssetEscrow` instruction payload.
    public static func cancelAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionPayloadBuilder.escrowOnlyInstruction(named: "CancelAssetEscrow",
                                                                       escrowId: escrowId)
    }

    /// Build an `OpenEscrowDispute` instruction payload.
    public static func openEscrowDispute(escrowId: String,
                                         evidenceHashes: [String] = []) throws -> NoritoJSON {
        var payload: [String: Any] = [
            "escrow_id": try NativeEscrowInstructionPayloadBuilder.normalized(escrowId, field: "escrowId"),
        ]
        let evidence = try NativeEscrowInstructionPayloadBuilder.normalizedEvidence(evidenceHashes)
        if !evidence.isEmpty {
            payload["evidence_hashes"] = evidence
        }
        return try NativeEscrowInstructionPayloadBuilder.instruction(named: "OpenEscrowDispute",
                                                                    payload: payload)
    }

    /// Build a `ResolveEscrowDispute` instruction payload.
    public static func resolveEscrowDispute(escrowId: String,
                                            buyerAmount: String,
                                            sellerAmount: String,
                                            evidenceHashes: [String] = []) throws -> NoritoJSON {
        var payload: [String: Any] = [
            "escrow_id": try NativeEscrowInstructionPayloadBuilder.normalized(escrowId, field: "escrowId"),
            "buyer_amount": try NativeEscrowInstructionPayloadBuilder.normalized(buyerAmount,
                                                                                 field: "buyerAmount"),
            "seller_amount": try NativeEscrowInstructionPayloadBuilder.normalized(sellerAmount,
                                                                                  field: "sellerAmount"),
        ]
        let evidence = try NativeEscrowInstructionPayloadBuilder.normalizedEvidence(evidenceHashes)
        if !evidence.isEmpty {
            payload["evidence_hashes"] = evidence
        }
        return try NativeEscrowInstructionPayloadBuilder.instruction(named: "ResolveEscrowDispute",
                                                                    payload: payload)
    }
}

public extension IrohaSDK {
    /// Build an `OpenAssetEscrow` instruction payload (Norito JSON).
    func buildOpenAssetEscrow(escrowId: String,
                              assetDefinition: String,
                              amount: String,
                              evidenceHashes: [String] = []) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.openAssetEscrow(escrowId: escrowId,
                                                            assetDefinition: assetDefinition,
                                                            amount: amount,
                                                            evidenceHashes: evidenceHashes)
    }

    /// Build an `AcceptAssetEscrow` instruction payload (Norito JSON).
    func buildAcceptAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.acceptAssetEscrow(escrowId: escrowId)
    }

    /// Build a `MarkEscrowPaymentSent` instruction payload (Norito JSON).
    func buildMarkEscrowPaymentSent(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.markEscrowPaymentSent(escrowId: escrowId)
    }

    /// Build a `ReleaseAssetEscrow` instruction payload (Norito JSON).
    func buildReleaseAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.releaseAssetEscrow(escrowId: escrowId)
    }

    /// Build a `CancelAssetEscrow` instruction payload (Norito JSON).
    func buildCancelAssetEscrow(escrowId: String) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.cancelAssetEscrow(escrowId: escrowId)
    }

    /// Build an `OpenEscrowDispute` instruction payload (Norito JSON).
    func buildOpenEscrowDispute(escrowId: String,
                                evidenceHashes: [String] = []) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.openEscrowDispute(escrowId: escrowId,
                                                              evidenceHashes: evidenceHashes)
    }

    /// Build a `ResolveEscrowDispute` instruction payload (Norito JSON).
    func buildResolveEscrowDispute(escrowId: String,
                                   buyerAmount: String,
                                   sellerAmount: String,
                                   evidenceHashes: [String] = []) throws -> NoritoJSON {
        try NativeEscrowInstructionBuilders.resolveEscrowDispute(escrowId: escrowId,
                                                                 buyerAmount: buyerAmount,
                                                                 sellerAmount: sellerAmount,
                                                                 evidenceHashes: evidenceHashes)
    }
}
