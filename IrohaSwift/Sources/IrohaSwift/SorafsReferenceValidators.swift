import Foundation

public enum SorafsReferenceValidationError: Error, Equatable {
    case bridgeUnavailable
    case invalidLabel(String)
    case invalidPrivateKey(String)
    case invalidOrderbookField(String)
    case unsupportedOrderbookPayloadKind(SorafsOrderbookPayloadKind)
}

public enum SorafsOrderbookPayloadKind: UInt32, Sendable {
    case orderRequest = 1
    case orderCancel = 2
    case tradeEvent = 3
    case settlementChannel = 4
    case settlementReceipt = 5
    case runtimeSnapshot = 6

    public var defaultLabel: String {
        switch self {
        case .orderRequest: return "order-request.to"
        case .orderCancel: return "order-cancel.to"
        case .tradeEvent: return "trade-event.to"
        case .settlementChannel: return "settlement-channel.to"
        case .settlementReceipt: return "settlement-receipt.to"
        case .runtimeSnapshot: return "orderbook-runtime-snapshot.to"
        }
    }

    public var isUserSignedPayload: Bool {
        switch self {
        case .orderRequest, .orderCancel, .settlementReceipt:
            return true
        case .tradeEvent, .settlementChannel, .runtimeSnapshot:
            return false
        }
    }
}

public enum SorafsPdpPayloadKind: UInt32, Sendable {
    case commitment = 1
    case challenge = 2
    case proof = 3

    public var defaultLabel: String {
        switch self {
        case .commitment: return "commitment.to"
        case .challenge: return "challenge.to"
        case .proof: return "proof.to"
        }
    }
}

public enum SorafsPopPayloadKind: UInt32, Sendable {
    case credential = 1
    case commitmentRoot = 2
    case revocationList = 3
    case enrollmentRequest = 4
    case renewalRequest = 5
    case membershipProof = 6
    case issuedCredentialBundle = 7

    public var defaultLabel: String {
        switch self {
        case .credential: return "pop-credential.to"
        case .commitmentRoot: return "pop-commitment-root.to"
        case .revocationList: return "pop-revocation-list.to"
        case .enrollmentRequest: return "pop-enrollment-request.to"
        case .renewalRequest: return "pop-renewal-request.to"
        case .membershipProof: return "pop-membership-proof.to"
        case .issuedCredentialBundle: return "pop-issued-credential-bundle.to"
        }
    }
}

public enum SorafsHedgingPayloadKind: UInt32, Sendable {
    case priceFeed = 1
    case referencePriceDecision = 2
    case billingLineItem = 3
    case billingStatement = 4

    public var defaultLabel: String {
        switch self {
        case .priceFeed: return "hedging-price-feed.to"
        case .referencePriceDecision: return "hedging-reference-price-decision.to"
        case .billingLineItem: return "billing-line-item.to"
        case .billingStatement: return "billing-statement.to"
        }
    }
}

public enum SorafsOrderbookSide: UInt32, Sendable {
    case bid = 1
    case ask = 2
}

public enum SorafsOrderbookTier: UInt32, Sendable {
    case hot = 1
    case warm = 2
    case archive = 3
}

public enum SorafsOrderbookCancelReason: UInt32, Sendable {
    case ownerRequested = 1
    case expired = 2
    case governance = 3
    case replaced = 4
}

public struct SorafsSignedOrderbookOrderRequestFields: Sendable {
    public let orderId: Data
    public let side: SorafsOrderbookSide
    public let tier: SorafsOrderbookTier
    public let pricePerGibMicroXor: String
    public let quantityGib: UInt64
    public let remainingGib: UInt64?
    public let ownerAccount: Data
    public let expiryUnix: UInt64
    public let nonce: UInt64
    public let makerFeeBps: UInt32
    public let takerFeeBps: UInt32

    public init(
        orderId: Data,
        side: SorafsOrderbookSide,
        tier: SorafsOrderbookTier,
        pricePerGibMicroXor: String,
        quantityGib: UInt64,
        remainingGib: UInt64? = nil,
        ownerAccount: Data,
        expiryUnix: UInt64,
        nonce: UInt64,
        makerFeeBps: UInt32,
        takerFeeBps: UInt32
    ) {
        self.orderId = orderId
        self.side = side
        self.tier = tier
        self.pricePerGibMicroXor = pricePerGibMicroXor
        self.quantityGib = quantityGib
        self.remainingGib = remainingGib
        self.ownerAccount = ownerAccount
        self.expiryUnix = expiryUnix
        self.nonce = nonce
        self.makerFeeBps = makerFeeBps
        self.takerFeeBps = takerFeeBps
    }
}

public struct SorafsSignedOrderbookOrderCancelFields: Sendable {
    public let orderId: Data
    public let ownerAccount: Data
    public let reason: SorafsOrderbookCancelReason
    public let nonce: UInt64

    public init(
        orderId: Data,
        ownerAccount: Data,
        reason: SorafsOrderbookCancelReason,
        nonce: UInt64
    ) {
        self.orderId = orderId
        self.ownerAccount = ownerAccount
        self.reason = reason
        self.nonce = nonce
    }
}

public struct SorafsSignedOrderbookSettlementReceiptFields: Sendable {
    public let receiptId: Data
    public let channelId: Data
    public let tradeId: Data
    public let rangeStart: UInt64
    public let rangeEnd: UInt64
    public let chunkHash: Data
    public let bytesDelivered: UInt64
    public let xorDebitedMicroXor: String
    public let providerCreditMicroXor: String
    public let feeAmountMicroXor: String
    public let issuedAtUnix: UInt64

    public init(
        receiptId: Data,
        channelId: Data,
        tradeId: Data,
        rangeStart: UInt64,
        rangeEnd: UInt64,
        chunkHash: Data,
        bytesDelivered: UInt64,
        xorDebitedMicroXor: String,
        providerCreditMicroXor: String,
        feeAmountMicroXor: String,
        issuedAtUnix: UInt64
    ) {
        self.receiptId = receiptId
        self.channelId = channelId
        self.tradeId = tradeId
        self.rangeStart = rangeStart
        self.rangeEnd = rangeEnd
        self.chunkHash = chunkHash
        self.bytesDelivered = bytesDelivered
        self.xorDebitedMicroXor = xorDebitedMicroXor
        self.providerCreditMicroXor = providerCreditMicroXor
        self.feeAmountMicroXor = feeAmountMicroXor
        self.issuedAtUnix = issuedAtUnix
    }
}

public enum SorafsReferenceValidators {
    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isSorafsReferenceValidationAvailable
    }

    public static var isOrderbookSigningAvailable: Bool {
        NoritoNativeBridge.shared.isSorafsReferenceOrderbookSigningAvailable
    }

    public static var isPopNativeAvailable: Bool {
        NoritoNativeBridge.shared.isSorafsReferencePopValidationAvailable
    }

    public static var isHedgingNativeAvailable: Bool {
        NoritoNativeBridge.shared.isSorafsReferenceHedgingValidationAvailable
    }

    public static var isOrderbookFieldBuilderAvailable: Bool {
        NoritoNativeBridge.shared.isSorafsReferenceOrderbookFieldBuilderAvailable
    }

    public static func validateOrderbookPayloadJSON(
        kind: SorafsOrderbookPayloadKind,
        payload: Data,
        label: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedLabel = try validatorLabel(label, fallback: kind.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidateOrderbook(
            kind: kind.rawValue,
            payload: payload,
            label: resolvedLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func validatePopPayloadJSON(
        kind: SorafsPopPayloadKind,
        payload: Data,
        label: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedLabel = try validatorLabel(label, fallback: kind.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidatePopPayload(
            kind: kind.rawValue,
            payload: payload,
            label: resolvedLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func validateHedgingPayloadJSON(
        kind: SorafsHedgingPayloadKind,
        payload: Data,
        label: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedLabel = try validatorLabel(label, fallback: kind.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidateHedgingPayload(
            kind: kind.rawValue,
            payload: payload,
            label: resolvedLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func signOrderbookPayload(
        kind: SorafsOrderbookPayloadKind,
        payload: Data,
        privateKey: Data
    ) throws -> Data {
        try requireUserSignedOrderbookKind(kind)
        try requirePrivateKey(privateKey)
        guard let signed = NoritoNativeBridge.shared.sorafsReferenceSignOrderbook(
            kind: kind.rawValue,
            payload: payload,
            privateKey: privateKey
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return signed
    }

    public static func buildSignedOrderbookOrderRequest(
        _ fields: SorafsSignedOrderbookOrderRequestFields,
        privateKey: Data
    ) throws -> Data {
        try requireFixed32(fields.orderId, "orderId")
        try requirePositive(fields.quantityGib, "quantityGib")
        let remainingGib = fields.remainingGib ?? fields.quantityGib
        try requirePositive(remainingGib, "remainingGib")
        try requireNonEmpty(fields.ownerAccount, "ownerAccount")
        try requirePositive(fields.expiryUnix, "expiryUnix")
        try requirePositive(fields.nonce, "nonce")
        try requireDecimal(fields.pricePerGibMicroXor, "pricePerGibMicroXor", positive: true)
        try requireFeeBps(fields.makerFeeBps, "makerFeeBps")
        try requireFeeBps(fields.takerFeeBps, "takerFeeBps")
        try requirePrivateKey(privateKey)
        let nativeFields = NativeSorafsOrderbookOrderRequestFields(
            orderId: fields.orderId,
            side: fields.side.rawValue,
            tier: fields.tier.rawValue,
            pricePerGibMicroXor: fields.pricePerGibMicroXor,
            quantityGib: fields.quantityGib,
            remainingGib: remainingGib,
            ownerAccount: fields.ownerAccount,
            expiryUnix: fields.expiryUnix,
            nonce: fields.nonce,
            makerFeeBps: fields.makerFeeBps,
            takerFeeBps: fields.takerFeeBps
        )
        guard let signed = NoritoNativeBridge.shared.sorafsReferenceBuildSignedOrderbookOrderRequest(
            fields: nativeFields,
            privateKey: privateKey
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return signed
    }

    public static func buildSignedOrderbookOrderCancel(
        _ fields: SorafsSignedOrderbookOrderCancelFields,
        privateKey: Data
    ) throws -> Data {
        try requireFixed32(fields.orderId, "orderId")
        try requireNonEmpty(fields.ownerAccount, "ownerAccount")
        try requirePositive(fields.nonce, "nonce")
        try requirePrivateKey(privateKey)
        let nativeFields = NativeSorafsOrderbookOrderCancelFields(
            orderId: fields.orderId,
            ownerAccount: fields.ownerAccount,
            reason: fields.reason.rawValue,
            nonce: fields.nonce
        )
        guard let signed = NoritoNativeBridge.shared.sorafsReferenceBuildSignedOrderbookOrderCancel(
            fields: nativeFields,
            privateKey: privateKey
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return signed
    }

    public static func buildSignedOrderbookSettlementReceipt(
        _ fields: SorafsSignedOrderbookSettlementReceiptFields,
        privateKey: Data
    ) throws -> Data {
        try requireFixed32(fields.receiptId, "receiptId")
        try requireFixed32(fields.channelId, "channelId")
        try requireFixed32(fields.tradeId, "tradeId")
        try requirePositive(fields.rangeEnd, "rangeEnd")
        try requireFixed32(fields.chunkHash, "chunkHash")
        try requirePositive(fields.bytesDelivered, "bytesDelivered")
        try requireDecimal(fields.xorDebitedMicroXor, "xorDebitedMicroXor", positive: true)
        try requireDecimal(fields.providerCreditMicroXor, "providerCreditMicroXor", positive: false)
        try requireDecimal(fields.feeAmountMicroXor, "feeAmountMicroXor", positive: false)
        try requirePositive(fields.issuedAtUnix, "issuedAtUnix")
        try requirePrivateKey(privateKey)
        let nativeFields = NativeSorafsOrderbookSettlementReceiptFields(
            receiptId: fields.receiptId,
            channelId: fields.channelId,
            tradeId: fields.tradeId,
            rangeStart: fields.rangeStart,
            rangeEnd: fields.rangeEnd,
            chunkHash: fields.chunkHash,
            bytesDelivered: fields.bytesDelivered,
            xorDebitedMicroXor: fields.xorDebitedMicroXor,
            providerCreditMicroXor: fields.providerCreditMicroXor,
            feeAmountMicroXor: fields.feeAmountMicroXor,
            issuedAtUnix: fields.issuedAtUnix
        )
        guard let signed = NoritoNativeBridge.shared.sorafsReferenceBuildSignedOrderbookSettlementReceipt(
            fields: nativeFields,
            privateKey: privateKey
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return signed
    }

    public static func validatePdpPayloadJSON(
        kind: SorafsPdpPayloadKind,
        payload: Data,
        label: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedLabel = try validatorLabel(label, fallback: kind.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidatePdpPayload(
            kind: kind.rawValue,
            payload: payload,
            label: resolvedLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func validatePdpCommitmentChallengeJSON(
        commitment: Data,
        challenge: Data,
        commitmentLabel: String? = nil,
        challengeLabel: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedCommitmentLabel = try validatorLabel(
            commitmentLabel,
            fallback: SorafsPdpPayloadKind.commitment.defaultLabel
        )
        let resolvedChallengeLabel = try validatorLabel(
            challengeLabel,
            fallback: SorafsPdpPayloadKind.challenge.defaultLabel
        )
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidatePdpCommitmentChallenge(
            commitment: commitment,
            commitmentLabel: resolvedCommitmentLabel,
            challenge: challenge,
            challengeLabel: resolvedChallengeLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func validatePdpChallengeProofJSON(
        challenge: Data,
        proof: Data,
        challengeLabel: String? = nil,
        proofLabel: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedChallengeLabel = try validatorLabel(
            challengeLabel,
            fallback: SorafsPdpPayloadKind.challenge.defaultLabel
        )
        let resolvedProofLabel = try validatorLabel(proofLabel, fallback: SorafsPdpPayloadKind.proof.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidatePdpChallengeProof(
            challenge: challenge,
            challengeLabel: resolvedChallengeLabel,
            proof: proof,
            proofLabel: resolvedProofLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    public static func validatePdpBundleJSON(
        commitment: Data,
        challenge: Data,
        proof: Data,
        commitmentLabel: String? = nil,
        challengeLabel: String? = nil,
        proofLabel: String? = nil,
        generatedAtUnix: UInt64 = currentEpochSeconds()
    ) throws -> String {
        let resolvedCommitmentLabel = try validatorLabel(
            commitmentLabel,
            fallback: SorafsPdpPayloadKind.commitment.defaultLabel
        )
        let resolvedChallengeLabel = try validatorLabel(
            challengeLabel,
            fallback: SorafsPdpPayloadKind.challenge.defaultLabel
        )
        let resolvedProofLabel = try validatorLabel(proofLabel, fallback: SorafsPdpPayloadKind.proof.defaultLabel)
        guard let json = NoritoNativeBridge.shared.sorafsReferenceValidatePdpBundle(
            commitment: commitment,
            commitmentLabel: resolvedCommitmentLabel,
            challenge: challenge,
            challengeLabel: resolvedChallengeLabel,
            proof: proof,
            proofLabel: resolvedProofLabel,
            generatedAtUnix: generatedAtUnix
        ) else {
            throw SorafsReferenceValidationError.bridgeUnavailable
        }
        return json
    }

    private static func currentEpochSeconds() -> UInt64 {
        let seconds = Date().timeIntervalSince1970
        return seconds > 0 ? UInt64(seconds) : 0
    }

    private static func validatorLabel(_ label: String?, fallback: String) throws -> String {
        let value = label ?? fallback
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw SorafsReferenceValidationError.invalidLabel("label must not be blank")
        }
        guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw SorafsReferenceValidationError.invalidLabel("label must not contain surrounding whitespace")
        }
        guard !value.contains("\u{0}") else {
            throw SorafsReferenceValidationError.invalidLabel("label must not contain NUL")
        }
        return value
    }

    private static func requireUserSignedOrderbookKind(_ kind: SorafsOrderbookPayloadKind) throws {
        guard kind.isUserSignedPayload else {
            throw SorafsReferenceValidationError.unsupportedOrderbookPayloadKind(kind)
        }
    }

    private static func requirePrivateKey(_ privateKey: Data) throws {
        guard privateKey.count == 32 else {
            throw SorafsReferenceValidationError.invalidPrivateKey("privateKey must be 32 bytes")
        }
        guard privateKey.contains(where: { $0 != 0 }) else {
            throw SorafsReferenceValidationError.invalidPrivateKey("privateKey must not be all zero")
        }
    }

    private static func requireFixed32(_ value: Data, _ field: String) throws {
        guard value.count == 32 else {
            throw SorafsReferenceValidationError.invalidOrderbookField("\(field) must be 32 bytes")
        }
    }

    private static func requireNonEmpty(_ value: Data, _ field: String) throws {
        guard !value.isEmpty else {
            throw SorafsReferenceValidationError.invalidOrderbookField("\(field) must not be empty")
        }
    }

    private static func requirePositive(_ value: UInt64, _ field: String) throws {
        guard value > 0 else {
            throw SorafsReferenceValidationError.invalidOrderbookField("\(field) must be greater than zero")
        }
    }

    private static func requireFeeBps(_ value: UInt32, _ field: String) throws {
        guard value <= UInt32(UInt16.max) else {
            throw SorafsReferenceValidationError.invalidOrderbookField("\(field) must fit in u16 basis points")
        }
    }

    private static func requireDecimal(_ value: String, _ field: String, positive: Bool) throws {
        guard !value.isEmpty, value.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else {
            throw SorafsReferenceValidationError.invalidOrderbookField(
                "\(field) must be an unsigned decimal integer"
            )
        }
        if positive && !value.utf8.contains(where: { $0 != 48 }) {
            throw SorafsReferenceValidationError.invalidOrderbookField("\(field) must be greater than zero")
        }
    }
}
