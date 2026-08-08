import Foundation

/// Fee component constrained by a signature-bound maximum charge.
public enum FeeChargeKind: UInt32, Sendable, CaseIterable, Comparable {
    case nexus = 0
    case pipelineGas = 1

    public static func < (lhs: FeeChargeKind, rhs: FeeChargeKind) -> Bool {
        lhs.rawValue < rhs.rawValue
    }

    fileprivate var wireName: String {
        switch self {
        case .nexus: return "nexus"
        case .pipelineGas: return "pipeline_gas"
        }
    }
}

extension FeeChargeKind: Codable {
    private enum CodingKeys: String, CodingKey, CaseIterable {
        case kind
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        guard try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "FeeChargeKind.value must be null"
            )
        }
        switch try container.decode(String.self, forKey: .kind) {
        case "nexus": self = .nexus
        case "pipeline_gas": self = .pipelineGas
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "FeeChargeKind.kind must be nexus or pipeline_gas"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(wireName, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }
}

/// Exact asset and maximum amount authorized for one fee component.
public struct FeeChargeLimit: Codable, Sendable, Equatable {
    public let kind: FeeChargeKind
    public let assetDefinitionId: String
    public let maxAmount: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case kind
        case assetDefinitionId = "asset_definition_id"
        case maxAmount = "max_amount"
    }

    public init(kind: FeeChargeKind, assetDefinitionId: String, maxAmount: String) throws {
        guard AssetDefinitionAddressCodec.canonicalDefinitionLiteral(assetDefinitionId) == assetDefinitionId else {
            throw FeePaymentIntentError.invalidAssetDefinitionId(assetDefinitionId)
        }
        let numeric: CanonicalNumericComponents
        do {
            numeric = try CanonicalNorito.parseNumeric(maxAmount)
        } catch {
            throw FeePaymentIntentError.invalidMaximumAmount(maxAmount)
        }
        guard numeric.canonicalString == maxAmount,
              numeric.canonicalNumeric.compared(to: CanonicalNumeric(isNegative: false, scale: 0, digits: "0")) == .orderedDescending else {
            throw FeePaymentIntentError.invalidMaximumAmount(maxAmount)
        }
        self.kind = kind
        self.assetDefinitionId = assetDefinitionId
        self.maxAmount = maxAmount
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        try self.init(
            kind: container.decode(FeeChargeKind.self, forKey: .kind),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            maxAmount: container.decode(String.self, forKey: .maxAmount)
        )
    }
}

/// Exact immutable sponsor-program identifier.
///
/// The sponsor remains in the canonical I105 form supplied by the caller,
/// including that literal's own chain discriminant. Norito carries the
/// domainless account controller, so the discriminant is validated at the
/// JSON/API boundary and is not rewritten into the controller payload.
public struct FeeSponsorProgramId: Codable, Sendable, Equatable, Hashable, CustomStringConvertible {
    public let sponsor: String
    public let name: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case sponsor
        case name
    }

    public init(sponsor: String, name: String) throws {
        _ = try canonicalFeeSponsorAddress(sponsor)
        guard isCanonicalFeeSponsorProgramName(name) else {
            throw FeePaymentIntentError.invalidProgramName(name)
        }
        self.sponsor = sponsor
        self.name = name
    }

    public init(_ literal: String) throws {
        guard literal == literal.trimmingCharacters(in: .whitespacesAndNewlines),
              let slash = literal.firstIndex(of: "/"),
              slash != literal.startIndex,
              slash == literal.lastIndex(of: "/"),
              literal.index(after: slash) != literal.endIndex else {
            throw FeePaymentIntentError.invalidProgramId(literal)
        }
        try self.init(
            sponsor: String(literal[..<slash]),
            name: String(literal[literal.index(after: slash)...])
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        try self.init(
            sponsor: container.decode(String.self, forKey: .sponsor),
            name: container.decode(String.self, forKey: .name)
        )
    }

    public var description: String { "\(sponsor)/\(name)" }
}

/// Required, signature-bound choice of fee payer, maxima, and executable gas bound.
public enum FeePaymentIntent: Sendable, Equatable {
    case authority(chargeLimits: [FeeChargeLimit], gasLimit: UInt64?)
    case sponsor(
        programId: FeeSponsorProgramId,
        programRevision: UInt64,
        chargeLimits: [FeeChargeLimit],
        gasLimit: UInt64?
    )

    public var chargeLimits: [FeeChargeLimit] {
        switch self {
        case let .authority(chargeLimits, _), let .sponsor(_, _, chargeLimits, _):
            return chargeLimits
        }
    }

    public var gasLimit: UInt64? {
        switch self {
        case let .authority(_, gasLimit), let .sponsor(_, _, _, gasLimit):
            return gasLimit
        }
    }

    public var sponsorProgram: (id: FeeSponsorProgramId, revision: UInt64)? {
        guard case let .sponsor(programId, programRevision, _, _) = self else { return nil }
        return (programId, programRevision)
    }

    /// Returns true only when a quote preserves the exact payer, revision, and gas bound.
    public func hasSamePayerAndGasBound(as other: FeePaymentIntent) -> Bool {
        guard gasLimit == other.gasLimit else { return false }
        switch (self, other) {
        case (.authority, .authority): return true
        case let (.sponsor(leftId, leftRevision, _, _), .sponsor(rightId, rightRevision, _, _)):
            return leftId == rightId && leftRevision == rightRevision
        default: return false
        }
    }

    /// Exact Norito JSON accepted by Torii and the native bridge.
    public func canonicalJSONData() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }

    /// Canonical (fixed-length field) Norito payload for `FeePaymentIntent`.
    public func canonicalNorito() throws -> Data {
        try encodeNorito(compact: false)
    }

    func compactNorito() throws -> Data { try encodeNorito(compact: true) }

    static func validate(chargeLimits: [FeeChargeLimit], gasLimit: UInt64?) throws {
        if gasLimit == 0 { throw FeePaymentIntentError.zeroGasLimit }
        var previous: FeeChargeKind?
        for limit in chargeLimits {
            if let previous, limit.kind <= previous {
                throw FeePaymentIntentError.nonCanonicalChargeLimits
            }
            previous = limit.kind
        }
    }
}

extension FeePaymentIntent: Codable {
    private enum CodingKeys: String, CodingKey, CaseIterable {
        case payer
        case value
    }

    private enum ValueKeys: String, CodingKey, CaseIterable {
        case programId = "program_id"
        case programRevision = "program_revision"
        case chargeLimits = "charge_limits"
        case gasLimit = "gas_limit"
    }

    public init(from decoder: Decoder) throws {
        let rawContainer = try decoder.container(keyedBy: FeePaymentDynamicCodingKey.self)
        try requireExactStringKeys(
            rawContainer,
            expected: ["payer", "value"],
            at: decoder.codingPath
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let payer = try container.decode(String.self, forKey: .payer)
        let valueDecoder = try container.superDecoder(forKey: .value)
        let value = try valueDecoder.container(keyedBy: ValueKeys.self)
        let rawValue = try valueDecoder.container(keyedBy: FeePaymentDynamicCodingKey.self)
        let limits = try value.decode([FeeChargeLimit].self, forKey: .chargeLimits)
        let gasLimit = try value.decodeIfPresent(UInt64.self, forKey: .gasLimit)
        switch payer {
        case "authority":
            try requireExactStringKeys(
                rawValue,
                expected: ["charge_limits", "gas_limit"],
                required: ["charge_limits"],
                at: decoder.codingPath + [CodingKeys.value]
            )
            try requireExactKeys(
                value,
                expected: [.chargeLimits, .gasLimit],
                required: [.chargeLimits],
                at: decoder.codingPath + [CodingKeys.value]
            )
            try FeePaymentIntent.validate(chargeLimits: limits, gasLimit: gasLimit)
            self = .authority(chargeLimits: limits, gasLimit: gasLimit)
        case "sponsor":
            try requireExactStringKeys(
                rawValue,
                expected: ["program_id", "program_revision", "charge_limits", "gas_limit"],
                required: ["program_id", "program_revision", "charge_limits"],
                at: decoder.codingPath + [CodingKeys.value]
            )
            try requireExactKeys(
                value,
                expected: Set(ValueKeys.allCases),
                required: [.programId, .programRevision, .chargeLimits],
                at: decoder.codingPath + [CodingKeys.value]
            )
            let revision = try value.decode(UInt64.self, forKey: .programRevision)
            guard revision > 0 else { throw FeePaymentIntentError.zeroProgramRevision }
            try FeePaymentIntent.validate(chargeLimits: limits, gasLimit: gasLimit)
            self = .sponsor(
                programId: try value.decode(FeeSponsorProgramId.self, forKey: .programId),
                programRevision: revision,
                chargeLimits: limits,
                gasLimit: gasLimit
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .payer,
                in: container,
                debugDescription: "FeePaymentIntent.payer must be authority or sponsor"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        var value = container.nestedContainer(keyedBy: ValueKeys.self, forKey: .value)
        switch self {
        case let .authority(chargeLimits, gasLimit):
            try FeePaymentIntent.validate(chargeLimits: chargeLimits, gasLimit: gasLimit)
            try container.encode("authority", forKey: .payer)
            try value.encode(chargeLimits, forKey: .chargeLimits)
            try value.encodeIfPresent(gasLimit, forKey: .gasLimit)
        case let .sponsor(programId, programRevision, chargeLimits, gasLimit):
            guard programRevision > 0 else { throw FeePaymentIntentError.zeroProgramRevision }
            try FeePaymentIntent.validate(chargeLimits: chargeLimits, gasLimit: gasLimit)
            try container.encode("sponsor", forKey: .payer)
            try value.encode(programId, forKey: .programId)
            try value.encode(programRevision, forKey: .programRevision)
            try value.encode(chargeLimits, forKey: .chargeLimits)
            try value.encodeIfPresent(gasLimit, forKey: .gasLimit)
        }
    }
}

private struct FeePaymentDynamicCodingKey: CodingKey, Hashable {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

public enum FeePaymentIntentError: Error, LocalizedError, Sendable, Equatable {
    case invalidAssetDefinitionId(String)
    case invalidMaximumAmount(String)
    case invalidSponsorAccount(String)
    case invalidProgramName(String)
    case invalidProgramId(String)
    case zeroProgramRevision
    case zeroGasLimit
    case nonCanonicalChargeLimits
    case quoteChangedPayerOrGas

    public var errorDescription: String? {
        switch self {
        case let .invalidAssetDefinitionId(value):
            return "Fee asset definition id must be an exact canonical public asset id: \(value)"
        case let .invalidMaximumAmount(value):
            return "Fee maximum must be a positive canonical quantity: \(value)"
        case let .invalidSponsorAccount(value):
            return "Fee sponsor must be an exact canonical I105 account id: \(value)"
        case let .invalidProgramName(value):
            return "Fee sponsor program name is invalid: \(value)"
        case let .invalidProgramId(value):
            return "Fee sponsor program id must use exact sponsor/program syntax: \(value)"
        case .zeroProgramRevision:
            return "Fee sponsor program revision must be positive."
        case .zeroGasLimit:
            return "Fee payment gas limit must be positive when present."
        case .nonCanonicalChargeLimits:
            return "Fee charge limits must be unique and ordered nexus before pipeline gas."
        case .quoteChangedPayerOrGas:
            return "Fee quote changed the selected payer, sponsor revision, or gas bound."
        }
    }
}

/// Consensus-visible lifecycle of a fee sponsor program.
public enum FeeSponsorProgramLifecycle: String, Codable, Sendable, Equatable {
    case staged
    case paused
    case active
    case closing
    case closed

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case state
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        guard try container.decodeNil(forKey: .value),
              let value = Self(rawValue: try container.decode(String.self, forKey: .state)) else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "Invalid fee sponsor program lifecycle"
            ))
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(rawValue, forKey: .state)
        try container.encodeNil(forKey: .value)
    }
}

/// Delayed activation of an immutable sponsor-program revision.
public struct FeeSponsorProgramActivation: Codable, Sendable, Equatable {
    public let revision: UInt64
    public let activateAtHeight: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case revision
        case activateAtHeight = "activate_at_height"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        revision = try container.decode(UInt64.self, forKey: .revision)
        activateAtHeight = try container.decode(UInt64.self, forKey: .activateAtHeight)
        guard revision > 0, activateAtHeight > 0 else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "Fee sponsor activation values must be positive"
            ))
        }
    }
}

/// Sponsor-owned lifecycle record returned by Torii.
public struct FeeSponsorProgram: Codable, Sendable, Equatable {
    public let id: FeeSponsorProgramId
    public let payoutAccount: String
    public let lifecycle: FeeSponsorProgramLifecycle
    public let activeRevision: UInt64?
    public let stagedRevision: UInt64?
    public let scheduledActivation: FeeSponsorProgramActivation?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case id
        case payoutAccount = "payout_account"
        case lifecycle
        case activeRevision = "active_revision"
        case stagedRevision = "staged_revision"
        case scheduledActivation = "scheduled_activation"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(
            container,
            expected: Set(CodingKeys.allCases),
            required: [.id, .payoutAccount, .lifecycle],
            at: decoder.codingPath
        )
        id = try container.decode(FeeSponsorProgramId.self, forKey: .id)
        payoutAccount = try container.decode(String.self, forKey: .payoutAccount)
        _ = try canonicalFeeSponsorAddress(payoutAccount)
        lifecycle = try container.decode(FeeSponsorProgramLifecycle.self, forKey: .lifecycle)
        activeRevision = try container.decodeIfPresent(UInt64.self, forKey: .activeRevision)
        stagedRevision = try container.decodeIfPresent(UInt64.self, forKey: .stagedRevision)
        scheduledActivation = try container.decodeIfPresent(
            FeeSponsorProgramActivation.self,
            forKey: .scheduledActivation
        )
        guard activeRevision.map({ $0 > 0 }) ?? true,
              stagedRevision.map({ $0 > 0 }) ?? true else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "Fee sponsor revisions must be positive"
            ))
        }
    }
}

/// Ledger observation fixing one deterministic fee quote.
public struct FeeQuoteObservation: Codable, Sendable, Equatable {
    public let ledgerTimeMs: UInt64
    public let nextBlockHeight: UInt64
    public let routeDataspaceId: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case ledgerTimeMs = "ledger_time_ms"
        case nextBlockHeight = "next_block_height"
        case routeDataspaceId = "route_dataspace_id"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        ledgerTimeMs = try container.decode(UInt64.self, forKey: .ledgerTimeMs)
        nextBlockHeight = try container.decode(UInt64.self, forKey: .nextBlockHeight)
        routeDataspaceId = try container.decode(UInt64.self, forKey: .routeDataspaceId)
        guard nextBlockHeight > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .nextBlockHeight,
                in: container,
                debugDescription: "next_block_height must be positive"
            )
        }
    }
}

/// One maximum fee component returned by a quote.
public struct FeeQuoteComponent: Codable, Sendable, Equatable {
    public let kind: FeeChargeKind
    public let assetDefinitionId: String
    public let maxAmount: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case kind
        case assetDefinitionId = "asset_definition_id"
        case maxAmount = "max_amount"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        kind = try container.decode(FeeChargeKind.self, forKey: .kind)
        assetDefinitionId = try container.decode(String.self, forKey: .assetDefinitionId)
        maxAmount = try container.decode(String.self, forKey: .maxAmount)
        try validateCanonicalAssetDefinition(assetDefinitionId)
        try validateCanonicalQuantity(maxAmount)
    }
}

/// Remaining sponsor capacity for one fee asset.
public struct FeeQuoteCapacity: Codable, Sendable, Equatable {
    public let assetDefinitionId: String
    public let vaultBalance: String
    public let reserveFloor: String
    public let blockRemaining: String
    public let programEpochRemaining: String
    public let beneficiaryEpochRemaining: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case assetDefinitionId = "asset_definition_id"
        case vaultBalance = "vault_balance"
        case reserveFloor = "reserve_floor"
        case blockRemaining = "block_remaining"
        case programEpochRemaining = "program_epoch_remaining"
        case beneficiaryEpochRemaining = "beneficiary_epoch_remaining"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        assetDefinitionId = try container.decode(String.self, forKey: .assetDefinitionId)
        vaultBalance = try container.decode(String.self, forKey: .vaultBalance)
        reserveFloor = try container.decode(String.self, forKey: .reserveFloor)
        blockRemaining = try container.decode(String.self, forKey: .blockRemaining)
        programEpochRemaining = try container.decode(String.self, forKey: .programEpochRemaining)
        beneficiaryEpochRemaining = try container.decode(String.self, forKey: .beneficiaryEpochRemaining)
        try validateCanonicalAssetDefinition(assetDefinitionId)
        for quantity in [
            vaultBalance, reserveFloor, blockRemaining,
            programEpochRemaining, beneficiaryEpochRemaining,
        ] {
            try validateCanonicalQuantity(quantity)
        }
    }
}

/// Account or isolated sponsor-program vault selected by admission.
public enum FeeDebitSource: Codable, Sendable, Equatable {
    case account(String)
    case sponsorProgram(FeeSponsorProgramId)

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case kind
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        switch try container.decode(String.self, forKey: .kind) {
        case "account":
            let account = try container.decode(String.self, forKey: .value)
            try validateCanonicalAccount(account)
            self = .account(account)
        case "sponsor_program":
            self = .sponsorProgram(try container.decode(FeeSponsorProgramId.self, forKey: .value))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "Fee debit source must be account or sponsor_program"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .account(account):
            try container.encode("account", forKey: .kind)
            try container.encode(account, forKey: .value)
        case let .sponsorProgram(programId):
            try container.encode("sponsor_program", forKey: .kind)
            try container.encode(programId, forKey: .value)
        }
    }
}

/// Successful deterministic fee-admission decision.
public struct FeeQuoteDecision: Codable, Sendable, Equatable {
    public let debitSource: FeeDebitSource
    public let programRevision: UInt64?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case status
        case value
    }

    private enum ValueKeys: String, CodingKey, CaseIterable {
        case debitSource = "debit_source"
        case programRevision = "program_revision"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        guard try container.decode(String.self, forKey: .status) == "accepted" else {
            throw DecodingError.dataCorruptedError(
                forKey: .status,
                in: container,
                debugDescription: "Fee quote decision must be accepted"
            )
        }
        let value = try container.nestedContainer(keyedBy: ValueKeys.self, forKey: .value)
        try requireExactKeys(
            value,
            expected: Set(ValueKeys.allCases),
            required: [.debitSource],
            at: decoder.codingPath + [CodingKeys.value]
        )
        debitSource = try value.decode(FeeDebitSource.self, forKey: .debitSource)
        programRevision = try value.decodeIfPresent(UInt64.self, forKey: .programRevision)
        guard programRevision.map({ $0 > 0 }) ?? true else {
            throw DecodingError.dataCorruptedError(
                forKey: .programRevision,
                in: value,
                debugDescription: "program_revision must be positive"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode("accepted", forKey: .status)
        var value = container.nestedContainer(keyedBy: ValueKeys.self, forKey: .value)
        try value.encode(debitSource, forKey: .debitSource)
        try value.encodeIfPresent(programRevision, forKey: .programRevision)
    }
}

/// Successful quote for an exact unsigned transaction payload.
public struct FeeQuoteResponse: Codable, Sendable, Equatable {
    public let intent: FeePaymentIntent
    public let observation: FeeQuoteObservation
    public let components: [FeeQuoteComponent]
    public let capacities: [FeeQuoteCapacity]
    public let decision: FeeQuoteDecision

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case intent
        case observation
        case components
        case capacities
        case decision
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try requireExactKeys(container, expected: Set(CodingKeys.allCases), at: decoder.codingPath)
        intent = try container.decode(FeePaymentIntent.self, forKey: .intent)
        observation = try container.decode(FeeQuoteObservation.self, forKey: .observation)
        components = try container.decode([FeeQuoteComponent].self, forKey: .components)
        capacities = try container.decode([FeeQuoteCapacity].self, forKey: .capacities)
        decision = try container.decode(FeeQuoteDecision.self, forKey: .decision)
    }

    /// Return the quoted maxima only if payer, immutable revision, and gas bound are unchanged.
    public func applying(to draft: FeePaymentIntent) throws -> FeePaymentIntent {
        guard draft.hasSamePayerAndGasBound(as: intent) else {
            throw FeePaymentIntentError.quoteChangedPayerOrGas
        }
        return intent
    }
}

private extension FeePaymentIntent {
    func encodeNorito(compact: Bool) throws -> Data {
        switch self {
        case let .authority(chargeLimits, gasLimit):
            try Self.validate(chargeLimits: chargeLimits, gasLimit: gasLimit)
            return try encodeEnum(tag: 0, compact: compact, body: encodeAuthority(
                chargeLimits: chargeLimits,
                gasLimit: gasLimit,
                compact: compact
            ))
        case let .sponsor(programId, programRevision, chargeLimits, gasLimit):
            guard programRevision > 0 else { throw FeePaymentIntentError.zeroProgramRevision }
            try Self.validate(chargeLimits: chargeLimits, gasLimit: gasLimit)
            return try encodeEnum(tag: 1, compact: compact, body: encodeSponsor(
                programId: programId,
                programRevision: programRevision,
                chargeLimits: chargeLimits,
                gasLimit: gasLimit,
                compact: compact
            ))
        }
    }

}

private func encodeEnum(tag: UInt32, compact: Bool, body: Data) throws -> Data {
    if compact {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(tag)
        writer.writeField(body)
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeUInt32LE(tag)
    writer.writeField(body)
    return writer.data
}

private func encodeAuthority(chargeLimits: [FeeChargeLimit], gasLimit: UInt64?, compact: Bool) throws -> Data {
    if compact {
        var writer = CompactNoritoWriter()
        writer.writeField(try encodeChargeLimits(chargeLimits, compact: true))
        writer.writeField(try CompactNorito.encodeOption(gasLimit, encode: CompactNorito.encodeUInt64))
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeField(try encodeChargeLimits(chargeLimits, compact: false))
    writer.writeField(try CanonicalNorito.encodeOption(gasLimit, encode: CanonicalNorito.encodeUInt64))
    return writer.data
}

private func encodeSponsor(
    programId: FeeSponsorProgramId,
    programRevision: UInt64,
    chargeLimits: [FeeChargeLimit],
    gasLimit: UInt64?,
    compact: Bool
) throws -> Data {
    if compact {
        var writer = CompactNoritoWriter()
        writer.writeField(try encodeProgramId(programId, compact: true))
        writer.writeField(CompactNorito.encodeUInt64(programRevision))
        writer.writeField(try encodeChargeLimits(chargeLimits, compact: true))
        writer.writeField(try CompactNorito.encodeOption(gasLimit, encode: CompactNorito.encodeUInt64))
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeField(try encodeProgramId(programId, compact: false))
    writer.writeField(CanonicalNorito.encodeUInt64(programRevision))
    writer.writeField(try encodeChargeLimits(chargeLimits, compact: false))
    writer.writeField(try CanonicalNorito.encodeOption(gasLimit, encode: CanonicalNorito.encodeUInt64))
    return writer.data
}

private func encodeProgramId(_ value: FeeSponsorProgramId, compact: Bool) throws -> Data {
    let address = try canonicalFeeSponsorAddress(value.sponsor)
    if compact {
        var writer = CompactNoritoWriter()
        writer.writeField(try address.compactNoritoAccountControllerPayload())
        writer.writeField(CompactNorito.encodeString(value.name))
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeField(try address.noritoAccountControllerPayload())
    writer.writeField(CanonicalNorito.encodeString(value.name))
    return writer.data
}

private func canonicalFeeSponsorAddress(_ value: String) throws -> AccountAddress {
    guard value == value.trimmingCharacters(in: .whitespacesAndNewlines),
          !value.isEmpty,
          !value.contains("@"),
          !value.contains("#"),
          !value.contains("$") else {
        throw FeePaymentIntentError.invalidSponsorAccount(value)
    }
    do {
        let prefix = try AccountAddress.inspectI105NetworkPrefix(value).chainDiscriminant
        let address = try AccountAddress.parseEncodedSwiftOnly(value, expectedPrefix: prefix)
        guard try address.toI105(networkPrefix: prefix) == value else {
            throw FeePaymentIntentError.invalidSponsorAccount(value)
        }
        return address
    } catch {
        throw FeePaymentIntentError.invalidSponsorAccount(value)
    }
}

/// Reject canonically-equivalent alternate UTF-8 spellings at every sponsor-program wire boundary.
func isCanonicalFeeSponsorProgramName(_ value: String) -> Bool {
    guard !value.isEmpty else { return false }
    let normalized = value.precomposedStringWithCanonicalMapping
    guard value.utf8.elementsEqual(normalized.utf8) else { return false }
    return value.unicodeScalars.allSatisfy { scalar in
        !CharacterSet.whitespacesAndNewlines.contains(scalar)
            && scalar != "@" && scalar != "#" && scalar != "$" && scalar != "/"
    }
}

private func encodeChargeLimits(_ values: [FeeChargeLimit], compact: Bool) throws -> Data {
    if compact {
        var writer = CompactNoritoWriter()
        // Norito's COMPACT_LEN flag applies to enclosing field and element
        // lengths, not to the sequence element count itself.
        writer.writeUInt64LE(UInt64(values.count))
        for value in values { writer.writeField(try encodeChargeLimit(value, compact: true)) }
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeLength(UInt64(values.count))
    for value in values { writer.writeField(try encodeChargeLimit(value, compact: false)) }
    return writer.data
}

private func encodeChargeLimit(_ value: FeeChargeLimit, compact: Bool) throws -> Data {
    guard let assetBytes = AssetDefinitionAddressCodec.uuidBytes(value.assetDefinitionId) else {
        throw FeePaymentIntentError.invalidAssetDefinitionId(value.assetDefinitionId)
    }
    if compact {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt32(value.kind.rawValue))
        writer.writeField(encodeFixedBytes(assetBytes, compact: true))
        writer.writeField(try encodeQuantity(value.maxAmount, compact: true))
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    writer.writeField(CanonicalNorito.encodeUInt32(value.kind.rawValue))
    writer.writeField(encodeFixedBytes(assetBytes, compact: false))
    writer.writeField(try CanonicalNorito.encodeNumeric(value.maxAmount))
    return writer.data
}

private func encodeFixedBytes(_ bytes: Data, compact: Bool) -> Data {
    if compact {
        var writer = CompactNoritoWriter()
        for byte in bytes { writer.writeLength(1); writer.writeUInt8(byte) }
        return writer.data
    }
    var writer = CanonicalNoritoWriter()
    for byte in bytes { writer.writeLength(1); writer.writeUInt8(byte) }
    return writer.data
}

private func encodeQuantity(_ value: String, compact: Bool) throws -> Data {
    guard compact else { return try CanonicalNorito.encodeNumeric(value) }
    let numeric = try CanonicalNorito.parseNumeric(value)
    let mantissaBytes = try numeric.mantissaBytes(maxBytes: CanonicalNorito.maxBigIntBytes)
    var bigint = CompactNoritoWriter()
    bigint.writeUInt32LE(UInt32(mantissaBytes.count))
    bigint.writeBytes(mantissaBytes)
    var writer = CompactNoritoWriter()
    writer.writeField(bigint.data)
    writer.writeField(CompactNorito.encodeUInt32(numeric.scale))
    return writer.data
}

private func requireExactKeys<Key: CodingKey & Hashable>(
    _ container: KeyedDecodingContainer<Key>,
    expected: Set<Key>,
    required: Set<Key>? = nil,
    at codingPath: [CodingKey]
) throws {
    let actual = Set(container.allKeys)
    guard actual.isSubset(of: expected) else {
        let unknown = actual.subtracting(expected).map(\.stringValue).sorted().joined(separator: ", ")
        throw DecodingError.dataCorrupted(.init(
            codingPath: codingPath,
            debugDescription: "Unknown fields: \(unknown)"
        ))
    }
    let missing = (required ?? expected).subtracting(actual)
    guard missing.isEmpty else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: codingPath,
            debugDescription: "Missing required fields: \(missing.map(\.stringValue).sorted().joined(separator: ", "))"
        ))
    }
}

private func requireExactStringKeys(
    _ container: KeyedDecodingContainer<FeePaymentDynamicCodingKey>,
    expected: Set<String>,
    required: Set<String>? = nil,
    at codingPath: [CodingKey]
) throws {
    let actual = Set(container.allKeys.map(\.stringValue))
    guard actual.isSubset(of: expected) else {
        let unknown = actual.subtracting(expected).sorted().joined(separator: ", ")
        throw DecodingError.dataCorrupted(.init(
            codingPath: codingPath,
            debugDescription: "Unknown fields: \(unknown)"
        ))
    }
    let missing = (required ?? expected).subtracting(actual)
    guard missing.isEmpty else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: codingPath,
            debugDescription: "Missing required fields: \(missing.sorted().joined(separator: ", "))"
        ))
    }
}

private func validateCanonicalAssetDefinition(_ value: String) throws {
    guard AssetDefinitionAddressCodec.canonicalDefinitionLiteral(value) == value else {
        throw FeePaymentIntentError.invalidAssetDefinitionId(value)
    }
}

private func validateCanonicalQuantity(_ value: String) throws {
    let numeric: CanonicalNumericComponents
    do {
        numeric = try CanonicalNorito.parseNumeric(value)
    } catch {
        throw FeePaymentIntentError.invalidMaximumAmount(value)
    }
    guard numeric.canonicalString == value,
          numeric.canonicalNumeric.compared(
              to: CanonicalNumeric(isNegative: false, scale: 0, digits: "0")
          ) != .orderedAscending else {
        throw FeePaymentIntentError.invalidMaximumAmount(value)
    }
}

private func validateCanonicalAccount(_ value: String) throws {
    // AccountId wire identity excludes the I105 display discriminant. Validate
    // the literal against its own canonical discriminant instead of rewriting
    // every accepted debit source through the SDK's default network.
    _ = try canonicalFeeSponsorAddress(value)
}
