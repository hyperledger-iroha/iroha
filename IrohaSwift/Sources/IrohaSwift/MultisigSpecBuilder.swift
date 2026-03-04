import Foundation

private let multisigSignatoryMax = 0xFF

/// Errors surfaced by ``MultisigSpecBuilder`` when the specification is invalid.
public enum MultisigSpecBuilderError: Error, LocalizedError, Equatable {
    case quorumNotSet
    case transactionTtlMissing
    case transactionTtlZero
    case noSignatories
    case signatoryLimitExceeded(Int)
    case quorumExceedsTotalWeight(quorum: UInt16, totalWeight: UInt32)

    public var errorDescription: String? {
        switch self {
        case .quorumNotSet:
            return "Multisig quorum must be configured before building a spec."
        case .transactionTtlMissing:
            return "Multisig specs require a transaction TTL."
        case .transactionTtlZero:
            return "Transaction TTL must be greater than zero."
        case .noSignatories:
            return "Multisig specs require at least one signatory."
        case let .signatoryLimitExceeded(count):
            return "Multisig specs support at most \(multisigSignatoryMax) signatories (found \(count))."
        case let .quorumExceedsTotalWeight(quorum, totalWeight):
            return "Quorum \(quorum) exceeds total signatory weight \(totalWeight)."
        }
    }
}

/// Errors surfaced when validating multisig proposal TTL overrides against the policy cap.
public enum MultisigProposalTtlError: Error, LocalizedError, Equatable {
    case overrideExceedsPolicy(requested: UInt64, cap: UInt64)

    public var errorDescription: String? {
        switch self {
        case let .overrideExceedsPolicy(requested, cap):
            return "Requested multisig TTL \(requested) ms exceeds the policy cap \(cap) ms; choose a value at or below the cap."
        }
    }
}

/// Codable payload matching `iroha_executor_data_model::isi::multisig::MultisigSpec`.
public struct MultisigSpecPayload: Codable, Sendable {
    public let signatories: [String: UInt8]
    public let quorum: UInt16
    public let transactionTtlMs: UInt64

    enum CodingKeys: String, CodingKey {
        case signatories
        case quorum
        case transactionTtlMs = "transaction_ttl_ms"
    }

    struct DynamicKey: CodingKey {
        var stringValue: String
        init?(stringValue: String) { self.stringValue = stringValue }
        var intValue: Int? { nil }
        init?(intValue _: Int) { nil }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        var map = container.nestedContainer(keyedBy: DynamicKey.self, forKey: .signatories)
        for account in signatories.keys.sorted() {
            let weight = signatories[account] ?? 0
            guard let key = DynamicKey(stringValue: account) else {
                continue
            }
            try map.encode(weight, forKey: key)
        }
        try container.encode(quorum, forKey: .quorum)
        try container.encode(transactionTtlMs, forKey: .transactionTtlMs)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let map = try container.nestedContainer(keyedBy: DynamicKey.self, forKey: .signatories)
        var decoded: [String: UInt8] = [:]
        for key in map.allKeys {
            decoded[key.stringValue] = try map.decode(UInt8.self, forKey: key)
        }
        self.signatories = decoded
        self.quorum = try container.decode(UInt16.self, forKey: .quorum)
        self.transactionTtlMs = try container.decode(UInt64.self, forKey: .transactionTtlMs)
    }

    public init(signatories: [String: UInt8], quorum: UInt16, transactionTtlMs: UInt64) {
        self.signatories = signatories
        self.quorum = quorum
        self.transactionTtlMs = transactionTtlMs
    }

    /// Encode the payload into JSON for Torii multisig registration.
    public func encodeJSON(prettyPrinted: Bool = false) throws -> Data {
        let encoder = JSONEncoder()
        if prettyPrinted {
            encoder.outputFormatting.insert(.prettyPrinted)
            encoder.outputFormatting.insert(.sortedKeys)
        }
        return try encoder.encode(self)
    }
}

/// Preview of a multisig proposal TTL using the policy cap as an upper bound.
public struct MultisigProposalTtlPreview: Equatable {
    /// TTL that will be applied to the proposal after enforcing the policy cap.
    public let effectiveTtlMs: UInt64
    /// Maximum TTL allowed by the multisig policy.
    public let policyCapMs: UInt64
    /// Approximate expiry timestamp in milliseconds since the Unix epoch (saturates at `UInt64.max` on overflow).
    public let expiresAtMs: UInt64
    /// `true` when a requested TTL exceeded the policy cap and was clamped.
    public let wasCapped: Bool
}

public extension MultisigSpecPayload {
    /// Returns a TTL preview for multisig proposals and relayers.
    ///
    /// The TTL is clamped to the policy's `transactionTtlMs` and the resulting
    /// expiry is computed relative to `now`. Use the `wasCapped` flag to surface
    /// UX hints when a requested TTL would be rejected by the node.
    /// - Parameters:
    ///   - requestedTtlMs: Optional TTL override for the proposal.
    ///   - now: Clock reference for the expiry calculation (defaults to `Date()`).
    /// - Returns: A `MultisigProposalTtlPreview` describing the effective TTL and expiry.
    func previewProposalExpiry(requestedTtlMs: UInt64?, now: Date = Date()) -> MultisigProposalTtlPreview {
        let cap = transactionTtlMs
        let requested = requestedTtlMs ?? cap
        let effective = min(requested, cap)
        let wasCapped = requested > cap

        let nowMs = now.timeIntervalSince1970 > 0
            ? UInt64(now.timeIntervalSince1970 * 1_000)
            : 0
        let (expiresAtMs, overflow) = nowMs.addingReportingOverflow(effective)

        return MultisigProposalTtlPreview(
            effectiveTtlMs: effective,
            policyCapMs: cap,
            expiresAtMs: overflow ? UInt64.max : expiresAtMs,
            wasCapped: wasCapped
        )
    }

    /// Validates a requested TTL against the policy cap, returning the preview when accepted.
    /// - Parameters:
    ///   - requestedTtlMs: Optional TTL override for the proposal.
    ///   - now: Clock reference for expiry calculations (defaults to `Date()`).
    /// - Throws: `MultisigProposalTtlError.overrideExceedsPolicy` when the override exceeds the policy cap.
    func enforceProposalExpiry(requestedTtlMs: UInt64?, now: Date = Date()) throws -> MultisigProposalTtlPreview {
        if let requested = requestedTtlMs, requested > transactionTtlMs {
            throw MultisigProposalTtlError.overrideExceedsPolicy(requested: requested,
                                                                 cap: transactionTtlMs)
        }
        return previewProposalExpiry(requestedTtlMs: requestedTtlMs, now: now)
    }
}

/// Builder for multisig registration specifications used by the IOS4 roadmap task.
public final class MultisigSpecBuilder: @unchecked Sendable {
    private var signatories: [String: UInt8] = [:]
    private var quorum: UInt16?
    private var transactionTtlMs: UInt64?

    public init() {}

    @discardableResult
    public func setQuorum(_ quorum: UInt16) -> MultisigSpecBuilder {
        self.quorum = quorum
        return self
    }

    @discardableResult
    public func setTransactionTtl(milliseconds ttl: UInt64) -> MultisigSpecBuilder {
        self.transactionTtlMs = ttl
        return self
    }

    @discardableResult
    public func addSignatory(accountId: String, weight: UInt8) -> MultisigSpecBuilder {
        signatories[accountId] = weight
        return self
    }

    @discardableResult
    public func removeSignatory(accountId: String) -> MultisigSpecBuilder {
        signatories.removeValue(forKey: accountId)
        return self
    }

    public func build() throws -> MultisigSpecPayload {
        guard let quorum else {
            throw MultisigSpecBuilderError.quorumNotSet
        }
        guard let ttl = transactionTtlMs else {
            throw MultisigSpecBuilderError.transactionTtlMissing
        }
        guard ttl > 0 else {
            throw MultisigSpecBuilderError.transactionTtlZero
        }
        guard !signatories.isEmpty else {
            throw MultisigSpecBuilderError.noSignatories
        }
        let count = signatories.count
        guard count <= multisigSignatoryMax else {
            throw MultisigSpecBuilderError.signatoryLimitExceeded(count)
        }
        let totalWeight = signatories.values.reduce(UInt32(0)) { partial, weight in
            partial &+ UInt32(weight)
        }
        guard totalWeight >= UInt32(quorum) else {
            throw MultisigSpecBuilderError.quorumExceedsTotalWeight(quorum: quorum,
                                                                    totalWeight: totalWeight)
        }

        return MultisigSpecPayload(signatories: signatories,
                                   quorum: quorum,
                                   transactionTtlMs: ttl)
    }

    public func encodeJSON(prettyPrinted: Bool = false) throws -> Data {
        let payload = try build()
        let encoder = JSONEncoder()
        if prettyPrinted {
            encoder.outputFormatting.insert(.prettyPrinted)
            encoder.outputFormatting.insert(.sortedKeys)
        }
        return try encoder.encode(payload)
    }
}
