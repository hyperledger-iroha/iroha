import CoreFoundation
import Foundation

/// Closed bridge payload kinds admitted in SCCP V1.
public enum SccpPayloadKindV1: String, CaseIterable, Sendable {
    case transfer
}

/// Request payload for `POST /v1/bridge/proofs/submit`.
public struct ToriiBridgeProofSubmitRequest: Encodable, Equatable, Sendable {
    public let authority: String
    public let feePayment: FeePaymentIntent
    public let signatureB64: String?
    public let transactionPayloadB64: String?
    public let destinationProofB64: String
    public let creationTimeMs: UInt64?

    public init(
        authority: String,
        destinationProofB64: String,
        signatureB64: String? = nil,
        transactionPayloadB64: String? = nil,
        creationTimeMs: UInt64? = nil,
        feePayment: FeePaymentIntent
    ) throws {
        let exactAuthority = try SccpSubmitValidation.authority(authority)
        self.authority = exactAuthority
        _ = try feePayment.compactNorito()
        self.feePayment = feePayment
        self.signatureB64 = try SccpSubmitValidation.optionalSignature(signatureB64)
        self.transactionPayloadB64 = try transactionPayloadB64.map {
            let payload = try SccpSubmitValidation.canonicalBase64(
                $0,
                field: "transaction_payload_b64",
                maximumBytes: SccpSubmitValidation.maximumTransactionPayloadBytes
            )
            try SccpSubmitValidation.canonicalTransactionPayload(
                payload,
                creationTimeMs: creationTimeMs,
                expectedAuthority: exactAuthority,
                expectedFeePayment: feePayment
            )
            return $0
        }
        _ = try SccpSubmitValidation.canonicalNoritoBase64(
            destinationProofB64,
            field: "destination_proof_b64",
            maximumBytes: SccpSubmitValidation.maximumDestinationArtifactBytes,
            expectedTypeName: SccpSubmitValidation.destinationArtifactTypeName
        )
        self.destinationProofB64 = destinationProofB64
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("creation_time_ms must be positive")
        }
        self.creationTimeMs = creationTimeMs
        try SccpSubmitValidation.detachedSigningState(
            signatureB64: self.signatureB64,
            transactionPayloadB64: self.transactionPayloadB64,
            creationTimeMs: creationTimeMs
        )
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case feePayment = "fee_payment"
        case signatureB64 = "signature_b64"
        case transactionPayloadB64 = "transaction_payload_b64"
        case destinationProofB64 = "destination_proof_b64"
        case creationTimeMs = "creation_time_ms"
    }
}

/// Native-proof-only request for `POST /v1/bridge/messages`.
public struct ToriiBridgeMessageSubmitRequest: Encodable, Equatable, Sendable {
    public let authority: String
    public let feePayment: FeePaymentIntent
    public let signatureB64: String?
    public let transactionPayloadB64: String?
    public let nativeProofB64: String
    public let creationTimeMs: UInt64?

    public init(
        authority: String,
        nativeProofB64: String,
        signatureB64: String? = nil,
        transactionPayloadB64: String? = nil,
        creationTimeMs: UInt64? = nil,
        feePayment: FeePaymentIntent
    ) throws {
        let exactAuthority = try SccpSubmitValidation.authority(authority)
        self.authority = exactAuthority
        _ = try feePayment.compactNorito()
        self.feePayment = feePayment
        self.signatureB64 = try SccpSubmitValidation.optionalSignature(signatureB64)
        self.transactionPayloadB64 = try transactionPayloadB64.map {
            let payload = try SccpSubmitValidation.canonicalBase64(
                $0,
                field: "transaction_payload_b64",
                maximumBytes: SccpSubmitValidation.maximumTransactionPayloadBytes
            )
            try SccpSubmitValidation.canonicalTransactionPayload(
                payload,
                creationTimeMs: creationTimeMs,
                expectedAuthority: exactAuthority,
                expectedFeePayment: feePayment
            )
            return $0
        }
        _ = try SccpSubmitValidation.canonicalNoritoBase64(
            nativeProofB64,
            field: "native_proof_b64",
            maximumBytes: SccpSubmitValidation.maximumNativeArtifactBytes,
            expectedTypeName: SccpSubmitValidation.nativeInboundProofTypeName
        )
        self.nativeProofB64 = nativeProofB64
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("creation_time_ms must be positive")
        }
        self.creationTimeMs = creationTimeMs
        try SccpSubmitValidation.detachedSigningState(
            signatureB64: self.signatureB64,
            transactionPayloadB64: self.transactionPayloadB64,
            creationTimeMs: creationTimeMs
        )
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case feePayment = "fee_payment"
        case signatureB64 = "signature_b64"
        case transactionPayloadB64 = "transaction_payload_b64"
        case nativeProofB64 = "native_proof_b64"
        case creationTimeMs = "creation_time_ms"
    }
}

/// Request-bound fields that can be checked against a bridge submit response.
public struct SccpBridgeResponseExpectation: Equatable, Sendable {
    public let payloadKind: SccpPayloadKindV1?
    public let messageIdHex: String?
    public let counterpartyDomain: UInt32?
    public let counterpartyChain: SccpNetworkV1?
    public let creationTimeMs: UInt64?

    public init(
        payloadKind: SccpPayloadKindV1? = nil,
        messageIdHex: String? = nil,
        counterpartyDomain: UInt32? = nil,
        counterpartyChain: SccpNetworkV1? = nil,
        creationTimeMs: UInt64? = nil
    ) throws {
        self.payloadKind = payloadKind
        self.messageIdHex = try messageIdHex.map {
            try SccpSubmitValidation.responseHash($0, field: "expected message_id_hex")
        }
        if let counterpartyDomain, !(1...5).contains(counterpartyDomain) {
            throw SccpV1Error.invalid("expected counterparty_domain must be in 1...5")
        }
        if let counterpartyChain, !counterpartyChain.isExternal {
            throw SccpV1Error.invalid("expected counterparty_chain must be external")
        }
        if let counterpartyDomain, let counterpartyChain,
           counterpartyDomain != counterpartyChain.domainId
        {
            throw SccpV1Error.invalid("expected counterparty profile/domain mismatch")
        }
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("expected creation_time_ms must be positive")
        }
        self.counterpartyDomain = counterpartyDomain
        self.counterpartyChain = counterpartyChain
        self.creationTimeMs = creationTimeMs
    }
}

/// Exact unified two-phase response returned by both SCCP submit endpoints.
public struct SccpBridgeSubmitResponse: Equatable, Sendable {
    public let submitted: Bool
    public let payloadKind: SccpPayloadKindV1
    public let messageIdHex: String
    public let backend: String
    public let counterpartyDomain: UInt32
    public let counterpartyChain: SccpNetworkV1
    public let routeConfigurationHashHex: String
    public let rangeStartHeight: UInt64
    public let rangeEndHeight: UInt64
    public let creationTimeMs: UInt64
    public let txHashHex: String?
    public let transactionPayloadB64: String?
    public let signingMessageB64: String?

    /// Strictly parse the exact response, rejecting duplicate, unknown, retired, or missing fields.
    public static func parse(
        _ data: Data,
        expectation: SccpBridgeResponseExpectation? = nil
    ) throws -> Self {
        let object = try SccpStrictJSON.object(data, label: "bridge submit response")
        let fields: Set<String> = [
            "submitted", "payload_kind", "message_id_hex", "backend",
            "counterparty_domain", "counterparty_chain", "route_configuration_hash_hex",
            "range_start_height", "range_end_height", "creation_time_ms", "tx_hash_hex",
            "transaction_payload_b64", "signing_message_b64",
        ]
        try SccpStrictJSON.exactFields(object, fields, label: "bridge submit response")
        let submitted = try SccpStrictJSON.boolean(object, "submitted")
        guard let payloadKind = SccpPayloadKindV1(rawValue: try SccpStrictJSON.text(object, "payload_kind")) else {
            throw SccpV1Error.invalid("payload_kind is unknown or retired")
        }
        let messageId = try SccpSubmitValidation.responseHash(
            SccpStrictJSON.text(object, "message_id_hex"),
            field: "message_id_hex"
        )
        let backend = try SccpStrictJSON.text(object, "backend")
        let exactBackends = Set(SccpNativeBackendV1.allCases.map(\.backendLabel)).union([
            "evm-groth16-bn254-v1",
            "tron-groth16-bn254-v1",
        ])
        guard exactBackends.contains(backend) else {
            throw SccpV1Error.invalid("backend must be one closed SCCP V1 backend label")
        }
        let domain = try SccpStrictJSON.uint32(object, "counterparty_domain", minimum: 1, maximum: 5)
        let chainKey = try SccpStrictJSON.text(object, "counterparty_chain")
        guard let chain = SccpNetworkV1(rawValue: chainKey), chain.isExternal, chain.domainId == domain else {
            throw SccpV1Error.invalid("counterparty_chain and counterparty_domain must identify one exact external network")
        }
        let backendsForDomain: Set<String>
        switch domain {
        case 1:
            backendsForDomain = [
                "evm-groth16-bn254-v1",
                "bridge/sccp/native/ethereum-beacon-v1",
            ]
        case 2:
            backendsForDomain = [
                "evm-groth16-bn254-v1",
                "bridge/sccp/native/bsc-parlia-v1",
            ]
        case 5:
            backendsForDomain = [
                "tron-groth16-bn254-v1",
                "bridge/sccp/native/tron-dpos-v1",
            ]
        default:
            backendsForDomain = []
        }
        guard backendsForDomain.contains(backend) else {
            throw SccpV1Error.invalid("backend does not match the exact counterparty family")
        }
        let routeConfigurationHash = try SccpSubmitValidation.responseHash(
            SccpStrictJSON.text(object, "route_configuration_hash_hex"),
            field: "route_configuration_hash_hex"
        )
        let start = try SccpStrictJSON.uint64(object, "range_start_height", minimum: 1)
        let end = try SccpStrictJSON.uint64(object, "range_end_height", minimum: start)
        let creation = try SccpStrictJSON.uint64(object, "creation_time_ms", minimum: 1)
        let txHash = try SccpStrictJSON.optionalText(object, "tx_hash_hex").map {
            try SccpSubmitValidation.responseHash($0, field: "tx_hash_hex")
        }
        let payloadB64 = try SccpStrictJSON.optionalText(object, "transaction_payload_b64")
        let signingB64 = try SccpStrictJSON.optionalText(object, "signing_message_b64")
        if submitted {
            guard txHash != nil, payloadB64 == nil, signingB64 == nil else {
                throw SccpV1Error.invalid("submitted SCCP response must contain tx_hash_hex and no signing payload")
            }
        } else {
            guard txHash == nil, let payloadB64, let signingB64 else {
                throw SccpV1Error.invalid("prepared SCCP response requires transaction_payload_b64 and signing_message_b64")
            }
            let payload = try SccpSubmitValidation.canonicalBase64(
                payloadB64,
                field: "transaction_payload_b64",
                maximumBytes: 16 * 1024 * 1024
            )
            try SccpSubmitValidation.canonicalTransactionPayload(
                payload,
                creationTimeMs: creation,
                expectedAuthority: nil,
                expectedFeePayment: nil
            )
            let signing = try SccpSubmitValidation.canonicalBase64(
                signingB64,
                field: "signing_message_b64",
                exactBytes: 32
            )
            var prehash = Blake2b.hash256(payload)
            prehash[prehash.count - 1] |= 1
            guard signing == prehash else {
                throw SccpV1Error.invalid("signing_message_b64 must be the exact transaction-payload prehash")
            }
        }
        let response = Self(
            submitted: submitted,
            payloadKind: payloadKind,
            messageIdHex: messageId,
            backend: backend,
            counterpartyDomain: domain,
            counterpartyChain: chain,
            routeConfigurationHashHex: routeConfigurationHash,
            rangeStartHeight: start,
            rangeEndHeight: end,
            creationTimeMs: creation,
            txHashHex: txHash,
            transactionPayloadB64: payloadB64,
            signingMessageB64: signingB64
        )
        try response.validate(expectation)
        return response
    }

    private func validate(_ expectation: SccpBridgeResponseExpectation?) throws {
        guard let expectation else { return }
        let checks: [(Bool, String)] = [
            (expectation.payloadKind == nil || expectation.payloadKind == payloadKind, "payload_kind"),
            (expectation.messageIdHex == nil || expectation.messageIdHex == messageIdHex, "message_id_hex"),
            (expectation.counterpartyDomain == nil || expectation.counterpartyDomain == counterpartyDomain, "counterparty_domain"),
            (expectation.counterpartyChain == nil || expectation.counterpartyChain == counterpartyChain, "counterparty_chain"),
            (expectation.creationTimeMs == nil || expectation.creationTimeMs == creationTimeMs, "creation_time_ms"),
        ]
        if let failed = checks.first(where: { !$0.0 }) {
            throw SccpV1Error.invalid("bridge submit response.\(failed.1) does not match the request")
        }
    }
}

enum SccpSubmitValidation {
    static let destinationArtifactTypeName =
        "iroha_sccp::SccpGroth16Bn254ProofArtifactV1"
    static let nativeInboundProofTypeName =
        "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1"
    static let registryTypeName =
        "iroha_data_model::bridge::sccp_registry::SccpRegistryV1"
    static let messageBundleTypeName = "iroha_sccp::TairaSccpMessageProofV1"
    static let proofRequestTypeName = "iroha_sccp::SccpGroth16Bn254ProofRequestV1"
    static let maximumNativeArtifactBytes = 16 * 1024 * 1024
    static let maximumDestinationArtifactBytes = maximumNativeArtifactBytes + 64 * 1024
    static let maximumDetachedSignatureBytes = 16 * 1024
    static let maximumTransactionPayloadBytes = 16 * 1024 * 1024
    static let maximumArtifactBytes = maximumDestinationArtifactBytes

    /// Require the canonical compact eight-field `TransactionPayload` layout used by SCCP.
    static func canonicalTransactionPayload(
        _ payload: Data,
        creationTimeMs: UInt64?,
        expectedAuthority: String?,
        expectedFeePayment: FeePaymentIntent?
    ) throws {
        var transaction = SccpCompactTransactionCursor(payload)
        let chain = try transaction.takeField("chain")
        let authority = try transaction.takeField("authority")
        let creation = try transaction.takeField("creation_time_ms")
        let executable = try transaction.takeField("instructions")
        let timeToLive = try transaction.takeField("time_to_live_ms")
        let nonce = try transaction.takeField("nonce")
        let feePayment = try transaction.takeField("fee_payment")
        let metadata = try transaction.takeField("metadata")
        guard transaction.isFinished, !chain.isEmpty, !authority.isEmpty, !executable.isEmpty,
              creation.count == MemoryLayout<UInt64>.size else {
            throw SccpV1Error.invalid(
                "transaction_payload_b64 must contain exactly one canonical eight-field TransactionPayload"
            )
        }
        let exactCreation = creation.withUnsafeBytes { bytes in
            UInt64(littleEndian: bytes.loadUnaligned(as: UInt64.self))
        }
        if let creationTimeMs, exactCreation != creationTimeMs {
            throw SccpV1Error.invalid(
                "transaction payload creation time does not match creation_time_ms"
            )
        }
        if let expectedAuthority {
            let address = try AccountAddress.parseEncoded(
                expectedAuthority,
                expectedPrefix: SccpV1.tairaI105DiscriminantV1
            )
            guard authority == (try address.compactNoritoAccountControllerPayload()) else {
                throw SccpV1Error.invalid(
                    "transaction payload authority does not match authority"
                )
            }
        }
        try requireAbsentCompactOption(timeToLive, field: "time_to_live_ms")
        try requireAbsentCompactOption(nonce, field: "nonce")
        let payloadFeeBinding = try requireCanonicalSccpFeePayment(feePayment)
        if let expectedFeePayment {
            let requestFeeBinding = try requireCanonicalSccpFeePayment(
                expectedFeePayment.compactNorito()
            )
            guard payloadFeeBinding == requestFeeBinding else {
                throw SccpV1Error.invalid(
                    "transaction payload fee_payment changed the request payer, sponsor program/revision, or gas bound"
                )
            }
        }
        var metadataCursor = SccpCompactTransactionCursor(metadata)
        guard try metadataCursor.takeLength("metadata.count") == 0,
              metadataCursor.isFinished else {
            throw SccpV1Error.invalid(
                "SCCP transaction metadata must be empty; fee selection belongs in fee_payment"
            )
        }
    }

    private static func requireAbsentCompactOption(_ payload: Data, field: String) throws {
        var cursor = SccpCompactTransactionCursor(payload)
        guard try cursor.takeByte(field) == 0, cursor.isFinished else {
            throw SccpV1Error.invalid(
                "SCCP transaction \(field) must use the exact None encoding"
            )
        }
    }

    private static func requireCanonicalSccpFeePayment(
        _ payload: Data
    ) throws -> SccpFeePaymentBinding {
        var intent = SccpCompactTransactionCursor(payload)
        let payer = try intent.takeUInt32("fee_payment.payer")
        var value = SccpCompactTransactionCursor(
            try intent.takeField("fee_payment.value")
        )
        let binding: SccpFeePaymentBinding
        switch payer {
        case 0:
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits")
            )
            let gasLimit = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit"
            )
            binding = SccpFeePaymentBinding(
                payer: payer,
                sponsorProgram: nil,
                programRevision: nil,
                gasLimit: gasLimit
            )
        case 1:
            let sponsorProgram = try value.takeField("fee_payment.program_id")
            try requireCanonicalSponsorProgram(sponsorProgram)
            let programRevision = try requireCanonicalPositiveUInt64(
                try value.takeField("fee_payment.program_revision"),
                field: "fee_payment.program_revision"
            )
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits")
            )
            let gasLimit = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit"
            )
            binding = SccpFeePaymentBinding(
                payer: payer,
                sponsorProgram: sponsorProgram,
                programRevision: programRevision,
                gasLimit: gasLimit
            )
        default:
            throw SccpV1Error.invalid(
                "SCCP transaction fee_payment contains an unknown payer variant"
            )
        }
        guard value.isFinished, intent.isFinished else {
            throw SccpV1Error.invalid("SCCP transaction fee_payment contains trailing bytes")
        }
        return binding
    }

    private static func requireCanonicalChargeLimits(_ payload: Data) throws {
        var limits = SccpCompactTransactionCursor(payload)
        let count = try limits.takeLength("fee_payment.charge_limits.count")
        guard count <= UInt64(FeeChargeKind.allCases.count) else {
            throw SccpV1Error.invalid(
                "fee_payment.charge_limits contains duplicate or unknown charge kinds"
            )
        }
        var previousKind: UInt32?
        for _ in 0..<count {
            var limit = SccpCompactTransactionCursor(
                try limits.takeField("fee_payment.charge_limits.item")
            )
            var kind = SccpCompactTransactionCursor(
                try limit.takeField("fee_payment.charge_limits.kind")
            )
            let rawKind = try kind.takeUInt32("fee_payment.charge_limits.kind")
            guard kind.isFinished, FeeChargeKind(rawValue: rawKind) != nil,
                  previousKind.map({ $0 < rawKind }) ?? true else {
                throw SccpV1Error.invalid(
                    "fee_payment.charge_limits must use unique canonical charge-kind order"
                )
            }
            previousKind = rawKind
            try requireCanonicalAssetDefinition(
                try limit.takeField("fee_payment.charge_limits.asset_definition_id")
            )
            try requireCanonicalPositiveQuantity(
                try limit.takeField("fee_payment.charge_limits.max_amount")
            )
            guard limit.isFinished else {
                throw SccpV1Error.invalid(
                    "fee_payment.charge_limits item contains trailing bytes"
                )
            }
        }
        guard limits.isFinished else {
            throw SccpV1Error.invalid("fee_payment.charge_limits contains trailing bytes")
        }
    }

    private static func requireCanonicalAssetDefinition(_ payload: Data) throws {
        var asset = SccpCompactTransactionCursor(payload)
        var bytes = Data()
        bytes.reserveCapacity(16)
        for _ in 0..<16 {
            guard try asset.takeLength("fee_payment.asset_definition_id.byte") == 1 else {
                throw SccpV1Error.invalid(
                    "fee_payment asset definition must use the exact 16-byte encoding"
                )
            }
            bytes.append(try asset.takeByte("fee_payment.asset_definition_id.byte"))
        }
        guard asset.isFinished, AssetDefinitionAddress.encode(uuidBytes: bytes) != nil else {
            throw SccpV1Error.invalid(
                "fee_payment asset definition must use the exact 16-byte encoding"
            )
        }
    }

    private static func requireCanonicalPositiveQuantity(_ payload: Data) throws {
        var quantity = SccpCompactTransactionCursor(payload)
        var mantissa = SccpCompactTransactionCursor(
            try quantity.takeField("fee_payment.max_amount.mantissa")
        )
        let byteCount = try mantissa.takeUInt32("fee_payment.max_amount.mantissa.count")
        guard byteCount > 0, byteCount <= UInt32(CanonicalNorito.maxBigIntBytes) else {
            throw SccpV1Error.invalid("fee_payment maximum must fit the canonical numeric bound")
        }
        let bytes = try mantissa.takeBytes(
            Int(byteCount),
            field: "fee_payment.max_amount.mantissa"
        )
        guard mantissa.isFinished,
              bytes.contains(where: { $0 != 0 }),
              let mostSignificant = bytes.last,
              mostSignificant & 0x80 == 0,
              bytes.count == 1 || mostSignificant != 0 || (bytes[bytes.count - 2] & 0x80) != 0 else {
            throw SccpV1Error.invalid(
                "fee_payment maximum must be a positive canonical quantity"
            )
        }
        var scale = SccpCompactTransactionCursor(
            try quantity.takeField("fee_payment.max_amount.scale")
        )
        guard try scale.takeUInt32("fee_payment.max_amount.scale") <= CanonicalNorito.maxNumericScale,
              scale.isFinished,
              quantity.isFinished else {
            throw SccpV1Error.invalid("fee_payment maximum contains an invalid numeric scale")
        }
    }

    private static func requireCanonicalSponsorProgram(_ payload: Data) throws {
        var program = SccpCompactTransactionCursor(payload)
        try requireCanonicalAccountController(
            try program.takeField("fee_payment.program_id.sponsor")
        )
        var name = SccpCompactTransactionCursor(
            try program.takeField("fee_payment.program_id.name")
        )
        let byteCount = try name.takeLength("fee_payment.program_id.name.length")
        guard byteCount > 0, byteCount <= UInt64(Int.max) else {
            throw SccpV1Error.invalid("fee_payment sponsor program name is invalid")
        }
        let nameBytes = try name.takeBytes(
            Int(byteCount),
            field: "fee_payment.program_id.name"
        )
        guard name.isFinished,
              program.isFinished,
              let value = String(data: nameBytes, encoding: .utf8),
              value == value.precomposedStringWithCanonicalMapping,
              value.unicodeScalars.allSatisfy({ scalar in
                  !CharacterSet.whitespacesAndNewlines.contains(scalar)
                      && scalar != "@" && scalar != "#" && scalar != "$" && scalar != "/"
              }) else {
            throw SccpV1Error.invalid("fee_payment sponsor program name is invalid")
        }
    }

    private static func requireCanonicalAccountController(_ payload: Data) throws {
        var controller = SccpCompactTransactionCursor(payload)
        let tag = try controller.takeUInt32("fee_payment.program_id.sponsor.controller")
        let body = try controller.takeField("fee_payment.program_id.sponsor.value")
        guard controller.isFinished else {
            throw SccpV1Error.invalid("fee_payment sponsor account contains trailing bytes")
        }
        switch tag {
        case 0:
            try requireCanonicalPublicKey(body)
        case 1:
            try requireCanonicalMultisigPolicy(body)
        default:
            throw SccpV1Error.invalid("fee_payment sponsor account controller is unknown")
        }
    }

    private static func requireCanonicalPublicKey(_ payload: Data) throws {
        var key = SccpCompactTransactionCursor(payload)
        let count = try key.takeUInt64("fee_payment.program_id.sponsor.public_key.count")
        guard count > 1, count <= 8_193 else {
            throw SccpV1Error.invalid("fee_payment sponsor public key length is invalid")
        }
        var bytes = Data()
        bytes.reserveCapacity(Int(count))
        for _ in 0..<count {
            guard try key.takeLength("fee_payment.program_id.sponsor.public_key.byte") == 1 else {
                throw SccpV1Error.invalid("fee_payment sponsor public key is not canonical")
            }
            bytes.append(try key.takeByte("fee_payment.program_id.sponsor.public_key.byte"))
        }
        guard key.isFinished,
              let algorithmByte = bytes.first,
              let algorithm = SigningAlgorithm(noritoDiscriminant: algorithmByte),
              (try? AccountAddress.fromAccount(
                  publicKey: Data(bytes.dropFirst()),
                  algorithm: algorithm.wireName
              )) != nil else {
            throw SccpV1Error.invalid("fee_payment sponsor public key is invalid")
        }
    }

    private static func requireCanonicalMultisigPolicy(_ payload: Data) throws {
        var policy = SccpCompactTransactionCursor(payload)
        var version = SccpCompactTransactionCursor(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.version")
        )
        guard try version.takeByte("fee_payment.program_id.sponsor.multisig.version") == 1,
              version.isFinished else {
            throw SccpV1Error.invalid("fee_payment sponsor multisig version is invalid")
        }
        var threshold = SccpCompactTransactionCursor(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.threshold")
        )
        let requiredWeight = try threshold.takeUInt16(
            "fee_payment.program_id.sponsor.multisig.threshold"
        )
        guard requiredWeight > 0, threshold.isFinished else {
            throw SccpV1Error.invalid("fee_payment sponsor multisig threshold is invalid")
        }
        var members = SccpCompactTransactionCursor(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.members")
        )
        let memberCount = try members.takeUInt64(
            "fee_payment.program_id.sponsor.multisig.members.count"
        )
        guard memberCount > 0, memberCount <= 1_024 else {
            throw SccpV1Error.invalid("fee_payment sponsor multisig member count is invalid")
        }
        var totalWeight: UInt64 = 0
        for _ in 0..<memberCount {
            var member = SccpCompactTransactionCursor(
                try members.takeField("fee_payment.program_id.sponsor.multisig.member")
            )
            try requireCanonicalPublicKey(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.public_key")
            )
            var weight = SccpCompactTransactionCursor(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.weight")
            )
            let value = try weight.takeUInt16(
                "fee_payment.program_id.sponsor.multisig.member.weight"
            )
            guard value > 0, weight.isFinished, member.isFinished else {
                throw SccpV1Error.invalid("fee_payment sponsor multisig member is invalid")
            }
            totalWeight += UInt64(value)
        }
        guard members.isFinished,
              policy.isFinished,
              UInt64(requiredWeight) <= totalWeight else {
            throw SccpV1Error.invalid("fee_payment sponsor multisig policy is invalid")
        }
    }

    private static func requireCanonicalPositiveUInt64(
        _ payload: Data,
        field: String
    ) throws -> UInt64 {
        var value = SccpCompactTransactionCursor(payload)
        let exact = try value.takeUInt64(field)
        guard exact > 0, value.isFinished else {
            throw SccpV1Error.invalid("\(field) must be a positive canonical UInt64")
        }
        return exact
    }

    private static func requireCanonicalPositiveUInt64Option(
        _ payload: Data,
        field: String
    ) throws -> UInt64? {
        var option = SccpCompactTransactionCursor(payload)
        switch try option.takeByte(field) {
        case 0:
            guard option.isFinished else {
                throw SccpV1Error.invalid("\(field) None encoding contains trailing bytes")
            }
            return nil
        case 1:
            let exact = try requireCanonicalPositiveUInt64(
                try option.takeField(field),
                field: field
            )
            guard option.isFinished else {
                throw SccpV1Error.invalid("\(field) Some encoding contains trailing bytes")
            }
            return exact
        default:
            throw SccpV1Error.invalid("\(field) contains an invalid option tag")
        }
    }

    static func authority(_ value: String) throws -> String {
        guard !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              let address = try? AccountAddress.parseEncoded(value),
              let canonical = try? address.toI105(
                  networkPrefix: SccpV1.tairaI105DiscriminantV1
              ),
              canonical == value
        else {
            throw SccpV1Error.invalid(
                "authority must be a canonical public Taira I105 AccountId (discriminant 369)"
            )
        }
        return value
    }

    static func optionalSignature(_ signatureB64: String?) throws -> String? {
        guard let signatureB64 else { return nil }
        let signature = try canonicalBase64(
            signatureB64,
            field: "signature_b64",
            maximumBytes: maximumDetachedSignatureBytes
        )
        guard signature.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("signature_b64 must contain one admitted nonzero signature payload")
        }
        return signatureB64
    }

    static func detachedSigningState(
        signatureB64: String?,
        transactionPayloadB64: String?,
        creationTimeMs: UInt64?
    ) throws {
        switch (signatureB64, transactionPayloadB64) {
        case (nil, nil):
            return
        case (.some, .some):
            guard creationTimeMs != nil else {
                throw SccpV1Error.invalid(
                    "signed SCCP submission requires an explicit positive creation_time_ms"
                )
            }
        default:
            throw SccpV1Error.invalid(
                "SCCP preparation requires neither signature_b64 nor transaction_payload_b64; signed submission requires both"
            )
        }
    }

    static func canonicalNoritoBase64(
        _ value: String,
        field: String,
        maximumBytes: Int,
        expectedTypeName: String
    ) throws -> Data {
        let data = try canonicalBase64(value, field: field, maximumBytes: maximumBytes)
        guard let frame = noritoDecodeFrame(data),
              frame.header.compression == .none,
              frame.header.schema == noritoSchemaHash(forTypeName: expectedTypeName),
              frame.paddingLength == 0,
              data.prefix(NoritoHeader.encodedLength) == frame.header.encode()
        else {
            throw SccpV1Error.invalid(
                "\(field) must contain the exact canonical uncompressed SCCP Norito type"
            )
        }
        return data
    }

    static func canonicalBase64(
        _ value: String,
        field: String,
        exactBytes: Int? = nil,
        maximumBytes: Int = maximumArtifactBytes
    ) throws -> Data {
        let byteBound = exactBytes ?? maximumBytes
        let encodedLengthBound = 4 * ((byteBound + 2) / 3)
        guard !value.isEmpty, value.utf8.count <= encodedLengthBound,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              let decoded = Data(base64Encoded: value),
              !decoded.isEmpty,
              decoded.count <= maximumBytes,
              decoded.base64EncodedString() == value
        else {
            throw SccpV1Error.invalid("\(field) must be canonical nonempty padded base64")
        }
        if let exactBytes, decoded.count != exactBytes {
            throw SccpV1Error.invalid("\(field) must contain exactly \(exactBytes) bytes")
        }
        return decoded
    }

    static func responseHash(_ value: String, field: String) throws -> String {
        guard value.count == 64,
              value.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              value.contains(where: { $0 != "0" })
        else {
            throw SccpV1Error.invalid("\(field) must be canonical lowercase nonzero 32-byte hex")
        }
        return value
    }

}

private struct SccpFeePaymentBinding: Equatable {
    let payer: UInt32
    let sponsorProgram: Data?
    let programRevision: UInt64?
    let gasLimit: UInt64?
}

private struct SccpCompactTransactionCursor {
    private let data: Data
    private var offset = 0

    init(_ data: Data) {
        self.data = data
    }

    var isFinished: Bool { offset == data.count }

    mutating func takeBytes(_ count: Int, field: String) throws -> Data {
        guard count >= 0, count <= data.count - offset else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        defer { offset += count }
        return data.subdata(in: offset..<(offset + count))
    }

    mutating func takeByte(_ field: String) throws -> UInt8 {
        guard offset < data.count else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        defer { offset += 1 }
        return data[offset]
    }

    mutating func takeLength(_ field: String) throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var count = 0
        while count < 10 {
            let byte = try takeByte(field)
            let chunk = UInt64(byte & 0x7f)
            guard shift < 64, chunk <= (UInt64.max >> shift) else {
                throw SccpV1Error.invalid("\(field) length overflows UInt64")
            }
            value |= chunk << shift
            count += 1
            if byte & 0x80 == 0 {
                guard count == 1 || chunk != 0 else {
                    throw SccpV1Error.invalid("\(field) length is not canonical")
                }
                return value
            }
            shift += 7
        }
        throw SccpV1Error.invalid("\(field) length is not canonical")
    }

    mutating func takeField(_ field: String) throws -> Data {
        let length = try takeLength(field)
        guard length <= UInt64(Int.max) else {
            throw SccpV1Error.invalid("\(field) exceeds the runtime bound")
        }
        let count = Int(length)
        guard count <= data.count - offset else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        defer { offset += count }
        return data.subdata(in: offset..<(offset + count))
    }

    mutating func takeUInt32(_ field: String) throws -> UInt32 {
        guard data.count - offset >= MemoryLayout<UInt32>.size else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        let value = data.subdata(in: offset..<(offset + MemoryLayout<UInt32>.size))
            .withUnsafeBytes { bytes in
                UInt32(littleEndian: bytes.loadUnaligned(as: UInt32.self))
            }
        offset += MemoryLayout<UInt32>.size
        return value
    }

    mutating func takeUInt16(_ field: String) throws -> UInt16 {
        guard data.count - offset >= MemoryLayout<UInt16>.size else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        let value = data.subdata(in: offset..<(offset + MemoryLayout<UInt16>.size))
            .withUnsafeBytes { bytes in
                UInt16(littleEndian: bytes.loadUnaligned(as: UInt16.self))
            }
        offset += MemoryLayout<UInt16>.size
        return value
    }

    mutating func takeUInt64(_ field: String) throws -> UInt64 {
        guard data.count - offset >= MemoryLayout<UInt64>.size else {
            throw SccpV1Error.invalid("\(field) is truncated")
        }
        let value = data.subdata(in: offset..<(offset + MemoryLayout<UInt64>.size))
            .withUnsafeBytes { bytes in
                UInt64(littleEndian: bytes.loadUnaligned(as: UInt64.self))
            }
        offset += MemoryLayout<UInt64>.size
        return value
    }
}

enum SccpStrictJSON {
    static func object(_ data: Data, label: String) throws -> [String: Any] {
        try rejectDuplicateKeys(data)
        guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw SccpV1Error.invalid("\(label) must be a JSON object")
        }
        return value
    }

    static func exactFields(_ value: [String: Any], _ fields: Set<String>, label: String) throws {
        try exactFields(value, allowed: fields, required: fields, label: label)
    }

    static func exactFields(
        _ value: [String: Any],
        allowed: Set<String>,
        required: Set<String>,
        label: String
    ) throws {
        if let unknown = value.keys.first(where: { !allowed.contains($0) }) {
            throw SccpV1Error.invalid("\(label) contains unknown or retired field `\(unknown)`")
        }
        if let missing = required.first(where: { value[$0] == nil }) {
            throw SccpV1Error.invalid("\(label) is missing required field `\(missing)`")
        }
    }

    static func text(_ value: [String: Any], _ field: String) throws -> String {
        guard let result = value[field] as? String, !result.isEmpty,
              result == result.trimmingCharacters(in: .whitespacesAndNewlines)
        else {
            throw SccpV1Error.invalid("\(field) must be canonical nonempty text")
        }
        return result
    }

    static func optionalText(_ value: [String: Any], _ field: String) throws -> String? {
        guard let raw = value[field], !(raw is NSNull) else { return nil }
        return try text(value, field)
    }

    static func boolean(_ value: [String: Any], _ field: String) throws -> Bool {
        guard let result = value[field] as? Bool else {
            throw SccpV1Error.invalid("\(field) must be boolean")
        }
        return result
    }

    static func uint32(
        _ value: [String: Any],
        _ field: String,
        minimum: UInt32,
        maximum: UInt32
    ) throws -> UInt32 {
        let number = try uint64(value, field, minimum: UInt64(minimum))
        guard number <= UInt64(maximum) else {
            throw SccpV1Error.invalid("\(field) is out of range")
        }
        return UInt32(number)
    }

    static func uint64(
        _ value: [String: Any],
        _ field: String,
        minimum: UInt64,
        maximum: UInt64 = UInt64.max
    ) throws -> UInt64 {
        guard let number = value[field] as? NSNumber,
              CFGetTypeID(number) != CFBooleanGetTypeID(),
              !CFNumberIsFloatType(number)
        else {
            throw SccpV1Error.invalid("\(field) must be a canonical unsigned JSON integer")
        }
        let result = number.uint64Value
        guard number.stringValue == String(result), result >= minimum, result <= maximum else {
            throw SccpV1Error.invalid("\(field) is out of range")
        }
        return result
    }

    private static func rejectDuplicateKeys(_ data: Data) throws {
        guard let text = String(data: data, encoding: .utf8), Data(text.utf8) == data else {
            throw SccpV1Error.invalid("JSON must be valid UTF-8")
        }
        var parser = DuplicateKeyParser(text)
        try parser.parse()
    }

    private struct DuplicateKeyParser {
        let text: String
        var index: String.Index

        init(_ text: String) {
            self.text = text
            index = text.startIndex
        }

        mutating func parse() throws {
            try value()
            whitespace()
            guard index == text.endIndex else { throw SccpV1Error.invalid("invalid JSON") }
        }

        mutating func value() throws {
            whitespace()
            guard let char = peek() else { throw SccpV1Error.invalid("invalid JSON") }
            switch char {
            case "{": try object()
            case "[": try array()
            case "\"": _ = try string()
            case "-", "0"..."9": try number()
            case "t": try consume("true")
            case "f": try consume("false")
            case "n": try consume("null")
            default: throw SccpV1Error.invalid("invalid JSON")
            }
        }

        mutating func object() throws {
            try consume("{")
            whitespace()
            var keys = Set<String>()
            if consumeIf("}") { return }
            while true {
                whitespace()
                let key = try string()
                guard keys.insert(key).inserted else {
                    throw SccpV1Error.invalid("JSON contains duplicate object key `\(key)`")
                }
                whitespace()
                try consume(":")
                try value()
                whitespace()
                if consumeIf("}") { return }
                try consume(",")
            }
        }

        mutating func array() throws {
            try consume("[")
            whitespace()
            if consumeIf("]") { return }
            while true {
                try value()
                whitespace()
                if consumeIf("]") { return }
                try consume(",")
            }
        }

        mutating func string() throws -> String {
            try consume("\"")
            var scalars = String.UnicodeScalarView()
            while let char = peek() {
                if char == "\"" {
                    advance()
                    return String(scalars)
                }
                if char == "\\" {
                    advance()
                    guard let escaped = peek() else { throw SccpV1Error.invalid("invalid JSON string") }
                    advance()
                    switch escaped {
                    case "\"", "\\", "/": scalars.append(escaped.unicodeScalars.first!)
                    case "b": scalars.append(UnicodeScalar(0x08)!)
                    case "f": scalars.append(UnicodeScalar(0x0c)!)
                    case "n": scalars.append(UnicodeScalar(0x0a)!)
                    case "r": scalars.append(UnicodeScalar(0x0d)!)
                    case "t": scalars.append(UnicodeScalar(0x09)!)
                    case "u": scalars.append(try unicodeEscape())
                    default: throw SccpV1Error.invalid("invalid JSON escape")
                    }
                } else {
                    guard char.unicodeScalars.allSatisfy({ $0.value >= 0x20 }) else {
                        throw SccpV1Error.invalid("invalid JSON string")
                    }
                    char.unicodeScalars.forEach { scalars.append($0) }
                    advance()
                }
            }
            throw SccpV1Error.invalid("unterminated JSON string")
        }

        mutating func unicodeEscape() throws -> UnicodeScalar {
            let high = try hexQuad()
            if (0xd800...0xdbff).contains(high) {
                guard consumeIf("\\"), consumeIf("u") else {
                    throw SccpV1Error.invalid("invalid JSON surrogate pair")
                }
                let low = try hexQuad()
                guard (0xdc00...0xdfff).contains(low),
                      let scalar = UnicodeScalar(0x10000 + ((high - 0xd800) << 10) + low - 0xdc00)
                else { throw SccpV1Error.invalid("invalid JSON surrogate pair") }
                return scalar
            }
            guard !(0xdc00...0xdfff).contains(high), let scalar = UnicodeScalar(high) else {
                throw SccpV1Error.invalid("invalid JSON Unicode escape")
            }
            return scalar
        }

        mutating func hexQuad() throws -> UInt32 {
            var result: UInt32 = 0
            for _ in 0..<4 {
                guard let char = peek(), let digit = char.hexDigitValue else {
                    throw SccpV1Error.invalid("invalid JSON Unicode escape")
                }
                result = result * 16 + UInt32(digit)
                advance()
            }
            return result
        }

        mutating func number() throws {
            guard let first = peek(), first.isNumber else { throw SccpV1Error.invalid("invalid JSON number") }
            if first == "0" {
                advance()
                if let next = peek(), next.isNumber { throw SccpV1Error.invalid("invalid JSON number") }
            } else {
                while let char = peek(), char.isNumber { advance() }
            }
            // SCCP V1 has no signed or fractional JSON fields. The scanner
            // deliberately leaves '.', 'e', and 'E' unconsumed so the
            // surrounding grammar rejects coercible spellings such as 1.0,
            // 1e0, or -0 before JSONSerialization loses their wire form.
        }

        mutating func whitespace() {
            while let char = peek(), " \n\r\t".contains(char) { advance() }
        }

        mutating func consume(_ literal: String) throws {
            guard text[index...].hasPrefix(literal) else { throw SccpV1Error.invalid("invalid JSON") }
            index = text.index(index, offsetBy: literal.count)
        }

        mutating func consumeIf(_ literal: String) -> Bool {
            guard text[index...].hasPrefix(literal) else { return false }
            index = text.index(index, offsetBy: literal.count)
            return true
        }

        func peek() -> Character? { index == text.endIndex ? nil : text[index] }
        mutating func advance() { index = text.index(after: index) }
    }
}
