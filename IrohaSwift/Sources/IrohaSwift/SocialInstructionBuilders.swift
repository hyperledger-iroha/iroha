import Foundation

/// Errors emitted by the social instruction helpers.
public enum SocialInstructionBuilderError: LocalizedError {
    case invalidDigest
    case invalidAmount

    public var errorDescription: String? {
        switch self {
        case .invalidDigest:
            return "bindingHash.digest must be a canonical Norito hash literal or 64‑character hexadecimal string"
        case .invalidAmount:
            return "amount must be a non-empty string representing a Numeric quantity"
        }
    }
}

/// Canonical keyed-hash payload used for SOC-2 viral incentive instructions.
///
/// The `digest` field is always stored as a canonical Norito hash literal of the form
/// `hash:<64-hex>#<4-hex-checksum>`.
public struct SocialKeyedHash: Equatable, Sendable {
    public let pepperId: String
    public let digest: String

    /// Create a keyed hash from a pepper id and digest.
    ///
    /// - Parameters:
    ///   - pepperId: Norito pepper identifier (e.g., `"pepper-social-v1"`).
    ///   - digest: Either a canonical Norito hash literal or a 64-character hexadecimal string.
    public init(pepperId: String, digest: String) throws {
        self.pepperId = pepperId
        self.digest = try SocialKeyedHash.canonicalHashLiteral(from: digest)
    }

    private static func canonicalHashLiteral(from raw: String) throws -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        if trimmed.lowercased().hasPrefix("hash:") {
            try validateHashLiteral(trimmed)
            return trimmed
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        let normalized = trimmed.uppercased()
        let checksum = crc16(tag: "hash", body: normalized)
        return "hash:\(normalized)#\(String(format: "%04X", checksum))"
    }

    private static func validateHashLiteral(_ literal: String) throws {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("hash:") else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        guard let separator = trimmed.lastIndex(of: "#") else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
        let body = String(trimmed[bodyStart..<separator])
        let checksum = String(trimmed[trimmed.index(after: separator)...])
        guard body.count == 64, Data(hexString: body) != nil else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        guard checksum.count == 4, Int(checksum, radix: 16) != nil else {
            throw SocialInstructionBuilderError.invalidDigest
        }
        let expected = crc16(tag: "hash", body: body.uppercased())
        let provided = UInt16(checksum, radix: 16) ?? 0
        guard expected == provided else {
            throw SocialInstructionBuilderError.invalidDigest
        }
    }

    private static func crc16(tag: String, body: String) -> UInt16 {
        var crc: UInt16 = 0xFFFF
        for byte in tag.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        crc = updateCrc(crc, value: Character(":").asciiValue ?? 0)
        for byte in body.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        return crc
    }

    private static func updateCrc(_ crc: UInt16, value: UInt8) -> UInt16 {
        var current = crc ^ UInt16(value) << 8
        for _ in 0..<8 {
            if current & 0x8000 != 0 {
                current = (current << 1) ^ 0x1021
            } else {
                current <<= 1
            }
        }
        return current & 0xFFFF
    }
}

private struct SocialKeyedHashJSON: Encodable, Equatable {
    let pepper_id: String
    let digest: String
}

private struct ClaimTwitterFollowRewardJSON: Encodable, Equatable {
    struct Inner: Encodable, Equatable {
        let binding_hash: SocialKeyedHashJSON
    }

    let ClaimTwitterFollowReward: Inner
}

private struct SendToTwitterJSON: Encodable, Equatable {
    struct Inner: Encodable, Equatable {
        let binding_hash: SocialKeyedHashJSON
        let amount: String
    }

    let SendToTwitter: Inner
}

private struct CancelTwitterEscrowJSON: Encodable, Equatable {
    struct Inner: Encodable, Equatable {
        let binding_hash: SocialKeyedHashJSON
    }

    let CancelTwitterEscrow: Inner
}

/// Swift helpers for building SOC-2 viral incentive instructions as Norito JSON payloads.
///
/// These mirror the JavaScript `instructionBuilders` helpers for:
/// `ClaimTwitterFollowReward`, `SendToTwitter`, and `CancelTwitterEscrow`.
public enum SocialInstructionBuilders {
    /// Build a `ClaimTwitterFollowReward` instruction Norito JSON payload.
    ///
    /// - Parameter binding: Keyed hash binding used by the soracles-based Twitter follow feed.
    /// - Returns: `NoritoJSON` encoding of `{ "ClaimTwitterFollowReward": { "binding_hash": { ... } } }`.
    public static func claimTwitterFollowReward(binding: SocialKeyedHash) throws -> NoritoJSON {
        let keyed = SocialKeyedHashJSON(pepper_id: binding.pepperId, digest: binding.digest)
        let payload = ClaimTwitterFollowRewardJSON(ClaimTwitterFollowReward: .init(binding_hash: keyed))
        return try NoritoJSON(payload)
    }

    /// Convenience overload that accepts a raw digest string.
    public static func claimTwitterFollowReward(pepperId: String, digest: String) throws -> NoritoJSON {
        let binding = try SocialKeyedHash(pepperId: pepperId, digest: digest)
        return try claimTwitterFollowReward(binding: binding)
    }

    /// Build a `SendToTwitter` instruction Norito JSON payload.
    ///
    /// - Parameters:
    ///   - binding: Keyed hash binding used by the soracles-based Twitter follow feed.
    ///   - amount: Decimal string representing the `Numeric` amount to send.
    /// - Returns: `NoritoJSON` encoding of `{ "SendToTwitter": { "binding_hash": { ... }, "amount": "<amount>" } }`.
    public static func sendToTwitter(binding: SocialKeyedHash, amount: String) throws -> NoritoJSON {
        let normalizedAmount = amount.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedAmount.isEmpty else {
            throw SocialInstructionBuilderError.invalidAmount
        }
        let keyed = SocialKeyedHashJSON(pepper_id: binding.pepperId, digest: binding.digest)
        let payload = SendToTwitterJSON(SendToTwitter: .init(binding_hash: keyed, amount: normalizedAmount))
        return try NoritoJSON(payload)
    }

    /// Convenience overload that accepts a raw digest string.
    public static func sendToTwitter(pepperId: String, digest: String, amount: String) throws -> NoritoJSON {
        let binding = try SocialKeyedHash(pepperId: pepperId, digest: digest)
        return try sendToTwitter(binding: binding, amount: amount)
    }

    /// Build a `CancelTwitterEscrow` instruction Norito JSON payload.
    ///
    /// - Parameter binding: Keyed hash binding identifying the escrow to cancel.
    /// - Returns: `NoritoJSON` encoding of `{ "CancelTwitterEscrow": { "binding_hash": { ... } } }`.
    public static func cancelTwitterEscrow(binding: SocialKeyedHash) throws -> NoritoJSON {
        let keyed = SocialKeyedHashJSON(pepper_id: binding.pepperId, digest: binding.digest)
        let payload = CancelTwitterEscrowJSON(CancelTwitterEscrow: .init(binding_hash: keyed))
        return try NoritoJSON(payload)
    }

    /// Convenience overload that accepts a raw digest string.
    public static func cancelTwitterEscrow(pepperId: String, digest: String) throws -> NoritoJSON {
        let binding = try SocialKeyedHash(pepperId: pepperId, digest: digest)
        return try cancelTwitterEscrow(binding: binding)
    }
}

