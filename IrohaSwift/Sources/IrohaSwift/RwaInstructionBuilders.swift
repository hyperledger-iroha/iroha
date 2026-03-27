import Foundation

/// Errors emitted by the RWA instruction helpers.
public enum RwaInstructionBuilderError: LocalizedError, Equatable {
    case invalidQuantity(field: String)
    case invalidJSONObject(field: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidQuantity(field):
            return "\(field) must be a non-empty string representing a Numeric quantity"
        case let .invalidJSONObject(field):
            return "\(field) must be a JSON object payload"
        }
    }
}

private enum RwaInstructionPayloadBuilder {
    static func normalizedQuantity(_ quantity: String, field: String) throws -> String {
        let trimmed = quantity.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) == nil else {
            throw RwaInstructionBuilderError.invalidQuantity(field: field)
        }
        return trimmed
    }

    static func jsonPayload(from rawJSON: String) throws -> NoritoJSON {
        try NoritoJSON(data: Data(rawJSON.trimmingCharacters(in: .whitespacesAndNewlines).utf8))
    }

    static func jsonObject(from payload: NoritoJSON, field: String) throws -> [String: Any] {
        let object = try JSONSerialization.jsonObject(with: payload.data, options: [.fragmentsAllowed])
        guard let map = object as? [String: Any] else {
            throw RwaInstructionBuilderError.invalidJSONObject(field: field)
        }
        return map
    }

    static func instruction(named name: String, payload: [String: Any]) throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([name: payload])
    }
}

/// Swift helpers for building dedicated RWA instruction payloads as Norito JSON.
///
/// The richer instructions accept canonical JSON objects so callers can pass the same
/// `NewRwa`, `MergeRwas`, and `RwaControlPolicy` shapes documented by the Rust data model
/// without introducing Swift-only wrapper types yet.
public enum RwaInstructionBuilders {
    /// Build a `RegisterRwa` instruction payload.
    public static func registerRwa(rwa: NoritoJSON) throws -> NoritoJSON {
        let rwaObject = try RwaInstructionPayloadBuilder.jsonObject(from: rwa, field: "rwa")
        return try RwaInstructionPayloadBuilder.instruction(named: "RegisterRwa", payload: ["rwa": rwaObject])
    }

    /// Convenience overload accepting a raw `NewRwa` JSON object.
    public static func registerRwa(rwaJSON: String) throws -> NoritoJSON {
        try registerRwa(rwa: RwaInstructionPayloadBuilder.jsonPayload(from: rwaJSON))
    }

    /// Build a `TransferRwa` instruction payload.
    public static func transferRwa(sourceAccountId: String,
                                   rwaId: String,
                                   quantity: String,
                                   destinationAccountId: String) throws -> NoritoJSON {
        let source = try TransactionInputValidator.sanitizeAccountId(sourceAccountId, field: "source")
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let normalizedQuantity = try RwaInstructionPayloadBuilder.normalizedQuantity(quantity, field: "quantity")
        let destination = try TransactionInputValidator.sanitizeAccountId(destinationAccountId, field: "destination")
        return try RwaInstructionPayloadBuilder.instruction(named: "TransferRwa", payload: [
            "source": source,
            "rwa": rwa,
            "quantity": normalizedQuantity,
            "destination": destination,
        ])
    }

    /// Build a `MergeRwas` instruction payload.
    public static func mergeRwas(merge: NoritoJSON) throws -> NoritoJSON {
        let mergeObject = try RwaInstructionPayloadBuilder.jsonObject(from: merge, field: "merge")
        return try RwaInstructionPayloadBuilder.instruction(named: "MergeRwas", payload: mergeObject)
    }

    /// Convenience overload accepting a raw `MergeRwas` JSON object.
    public static func mergeRwas(mergeJSON: String) throws -> NoritoJSON {
        try mergeRwas(merge: RwaInstructionPayloadBuilder.jsonPayload(from: mergeJSON))
    }

    /// Build a `RedeemRwa` instruction payload.
    public static func redeemRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let normalizedQuantity = try RwaInstructionPayloadBuilder.normalizedQuantity(quantity, field: "quantity")
        return try RwaInstructionPayloadBuilder.instruction(named: "RedeemRwa", payload: [
            "rwa": rwa,
            "quantity": normalizedQuantity,
        ])
    }

    /// Build a `FreezeRwa` instruction payload.
    public static func freezeRwa(rwaId: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        return try RwaInstructionPayloadBuilder.instruction(named: "FreezeRwa", payload: ["rwa": rwa])
    }

    /// Build an `UnfreezeRwa` instruction payload.
    public static func unfreezeRwa(rwaId: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        return try RwaInstructionPayloadBuilder.instruction(named: "UnfreezeRwa", payload: ["rwa": rwa])
    }

    /// Build a `HoldRwa` instruction payload.
    public static func holdRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let normalizedQuantity = try RwaInstructionPayloadBuilder.normalizedQuantity(quantity, field: "quantity")
        return try RwaInstructionPayloadBuilder.instruction(named: "HoldRwa", payload: [
            "rwa": rwa,
            "quantity": normalizedQuantity,
        ])
    }

    /// Build a `ReleaseRwa` instruction payload.
    public static func releaseRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let normalizedQuantity = try RwaInstructionPayloadBuilder.normalizedQuantity(quantity, field: "quantity")
        return try RwaInstructionPayloadBuilder.instruction(named: "ReleaseRwa", payload: [
            "rwa": rwa,
            "quantity": normalizedQuantity,
        ])
    }

    /// Build a `ForceTransferRwa` instruction payload.
    public static func forceTransferRwa(rwaId: String,
                                        quantity: String,
                                        destinationAccountId: String) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let normalizedQuantity = try RwaInstructionPayloadBuilder.normalizedQuantity(quantity, field: "quantity")
        let destination = try TransactionInputValidator.sanitizeAccountId(destinationAccountId, field: "destination")
        return try RwaInstructionPayloadBuilder.instruction(named: "ForceTransferRwa", payload: [
            "rwa": rwa,
            "quantity": normalizedQuantity,
            "destination": destination,
        ])
    }

    /// Build a `SetRwaControls` instruction payload.
    public static func setRwaControls(rwaId: String, controls: NoritoJSON) throws -> NoritoJSON {
        let rwa = try TransactionInputValidator.sanitizeRwaId(rwaId, field: "rwa")
        let controlsObject = try RwaInstructionPayloadBuilder.jsonObject(from: controls, field: "controls")
        return try RwaInstructionPayloadBuilder.instruction(named: "SetRwaControls", payload: [
            "rwa": rwa,
            "controls": controlsObject,
        ])
    }

    /// Convenience overload accepting a raw `RwaControlPolicy` JSON object.
    public static func setRwaControls(rwaId: String, controlsJSON: String) throws -> NoritoJSON {
        try setRwaControls(rwaId: rwaId, controls: RwaInstructionPayloadBuilder.jsonPayload(from: controlsJSON))
    }
}

public extension IrohaSDK {
    /// Build a `RegisterRwa` instruction payload (Norito JSON).
    func buildRegisterRwa(rwa: NoritoJSON) throws -> NoritoJSON {
        try RwaInstructionBuilders.registerRwa(rwa: rwa)
    }

    /// Convenience overload accepting a raw `NewRwa` JSON object.
    func buildRegisterRwa(rwaJSON: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.registerRwa(rwaJSON: rwaJSON)
    }

    /// Build a `TransferRwa` instruction payload (Norito JSON).
    func buildTransferRwa(sourceAccountId: String,
                          rwaId: String,
                          quantity: String,
                          destinationAccountId: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.transferRwa(sourceAccountId: sourceAccountId,
                                               rwaId: rwaId,
                                               quantity: quantity,
                                               destinationAccountId: destinationAccountId)
    }

    /// Build a `MergeRwas` instruction payload (Norito JSON).
    func buildMergeRwas(merge: NoritoJSON) throws -> NoritoJSON {
        try RwaInstructionBuilders.mergeRwas(merge: merge)
    }

    /// Convenience overload accepting a raw `MergeRwas` JSON object.
    func buildMergeRwas(mergeJSON: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.mergeRwas(mergeJSON: mergeJSON)
    }

    /// Build a `RedeemRwa` instruction payload (Norito JSON).
    func buildRedeemRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.redeemRwa(rwaId: rwaId, quantity: quantity)
    }

    /// Build a `FreezeRwa` instruction payload (Norito JSON).
    func buildFreezeRwa(rwaId: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.freezeRwa(rwaId: rwaId)
    }

    /// Build an `UnfreezeRwa` instruction payload (Norito JSON).
    func buildUnfreezeRwa(rwaId: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.unfreezeRwa(rwaId: rwaId)
    }

    /// Build a `HoldRwa` instruction payload (Norito JSON).
    func buildHoldRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.holdRwa(rwaId: rwaId, quantity: quantity)
    }

    /// Build a `ReleaseRwa` instruction payload (Norito JSON).
    func buildReleaseRwa(rwaId: String, quantity: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.releaseRwa(rwaId: rwaId, quantity: quantity)
    }

    /// Build a `ForceTransferRwa` instruction payload (Norito JSON).
    func buildForceTransferRwa(rwaId: String,
                               quantity: String,
                               destinationAccountId: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.forceTransferRwa(rwaId: rwaId,
                                                    quantity: quantity,
                                                    destinationAccountId: destinationAccountId)
    }

    /// Build a `SetRwaControls` instruction payload (Norito JSON).
    func buildSetRwaControls(rwaId: String, controls: NoritoJSON) throws -> NoritoJSON {
        try RwaInstructionBuilders.setRwaControls(rwaId: rwaId, controls: controls)
    }

    /// Convenience overload accepting a raw `RwaControlPolicy` JSON object.
    func buildSetRwaControls(rwaId: String, controlsJSON: String) throws -> NoritoJSON {
        try RwaInstructionBuilders.setRwaControls(rwaId: rwaId, controlsJSON: controlsJSON)
    }
}
