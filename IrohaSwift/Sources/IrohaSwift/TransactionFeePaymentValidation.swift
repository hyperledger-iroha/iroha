import Foundation

/// Structural checks for the compact `FeePaymentIntent` and metadata fields of a
/// canonical `TransactionPayload`.
enum TransactionFeePaymentValidation {
    /// Validates the canonical compact `FeePaymentIntent` shared by transaction payloads.
    static func requireCanonicalTransactionFeePayment(_ payload: Data) throws {
        var intent = ToriiVerifyingKeyCompactReader(payload)
        let payer = try intent.takeUInt32("fee_payment.payer")
        var value = ToriiVerifyingKeyCompactReader(
            try intent.takeField("fee_payment.value")
        )
        switch payer {
        case 0:
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits")
            )
            _ = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit"
            )
        case 1:
            try requireCanonicalSponsorProgram(
                try value.takeField("fee_payment.program_id")
            )
            _ = try requireCanonicalPositiveUInt64(
                try value.takeField("fee_payment.program_revision"),
                field: "fee_payment.program_revision"
            )
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits")
            )
            _ = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit"
            )
        default:
            throw invalid("fee_payment contains an unknown payer variant")
        }
        guard value.isFinished, intent.isFinished else {
            throw invalid("fee_payment contains trailing bytes")
        }
    }

    /// Requires the exact empty compact metadata encoding used when a request has no metadata.
    static func requireEmptyTransactionMetadata(_ payload: Data) throws {
        var metadata = ToriiVerifyingKeyCompactReader(payload)
        guard try metadata.takeUInt64("metadata.count") == 0,
              metadata.isFinished else {
            throw invalid("transaction metadata must use the exact empty encoding")
        }
    }

    private static func requireCanonicalChargeLimits(_ payload: Data) throws {
        var limits = ToriiVerifyingKeyCompactReader(payload)
        // Norito's compact-length flag applies to enclosing fields and
        // elements, while sequence counts remain fixed-width little-endian.
        let count = try limits.takeUInt64("fee_payment.charge_limits.count")
        guard count <= UInt64(FeeChargeKind.allCases.count) else {
            throw invalid("fee_payment.charge_limits contains duplicate or unknown charge kinds")
        }
        var previousKind: UInt32?
        for _ in 0..<count {
            var limit = ToriiVerifyingKeyCompactReader(
                try limits.takeField("fee_payment.charge_limits.item")
            )
            var kind = ToriiVerifyingKeyCompactReader(
                try limit.takeField("fee_payment.charge_limits.kind")
            )
            let rawKind = try kind.takeUInt32("fee_payment.charge_limits.kind")
            guard kind.isFinished, FeeChargeKind(rawValue: rawKind) != nil,
                  previousKind.map({ $0 < rawKind }) ?? true else {
                throw invalid("fee_payment.charge_limits must use unique canonical charge-kind order")
            }
            previousKind = rawKind
            try requireCanonicalAssetDefinition(
                try limit.takeField("fee_payment.charge_limits.asset_definition_id")
            )
            try requireCanonicalPositiveQuantity(
                try limit.takeField("fee_payment.charge_limits.max_amount")
            )
            guard limit.isFinished else {
                throw invalid("fee_payment.charge_limits item contains trailing bytes")
            }
        }
        guard limits.isFinished else {
            throw invalid("fee_payment.charge_limits contains trailing bytes")
        }
    }

    private static func requireCanonicalAssetDefinition(_ payload: Data) throws {
        var asset = ToriiVerifyingKeyCompactReader(payload)
        var bytes = Data()
        bytes.reserveCapacity(16)
        for _ in 0..<16 {
            guard try asset.takeLength("fee_payment.asset_definition_id.byte") == 1 else {
                throw invalid("fee_payment asset definition must use the exact 16-byte encoding")
            }
            bytes.append(try asset.takeUInt8("fee_payment.asset_definition_id.byte"))
        }
        guard asset.isFinished, AssetDefinitionAddress.encode(uuidBytes: bytes) != nil else {
            throw invalid("fee_payment asset definition must use the exact 16-byte encoding")
        }
    }

    private static func requireCanonicalPositiveQuantity(_ payload: Data) throws {
        var quantity = ToriiVerifyingKeyCompactReader(payload)
        var mantissa = ToriiVerifyingKeyCompactReader(
            try quantity.takeField("fee_payment.max_amount.mantissa")
        )
        let byteCount = try mantissa.takeUInt32("fee_payment.max_amount.mantissa.count")
        guard byteCount > 0, byteCount <= UInt32(CanonicalNorito.maxBigIntBytes) else {
            throw invalid("fee_payment maximum must fit the canonical numeric bound")
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
            throw invalid("fee_payment maximum must be a positive canonical quantity")
        }
        var scale = ToriiVerifyingKeyCompactReader(
            try quantity.takeField("fee_payment.max_amount.scale")
        )
        guard try scale.takeUInt32("fee_payment.max_amount.scale") <= CanonicalNorito.maxNumericScale,
              scale.isFinished,
              quantity.isFinished else {
            throw invalid("fee_payment maximum contains an invalid numeric scale")
        }
    }

    private static func requireCanonicalSponsorProgram(_ payload: Data) throws {
        var program = ToriiVerifyingKeyCompactReader(payload)
        try requireCanonicalAccountController(
            try program.takeField("fee_payment.program_id.sponsor")
        )
        var name = ToriiVerifyingKeyCompactReader(
            try program.takeField("fee_payment.program_id.name")
        )
        let byteCount = try name.takeLength("fee_payment.program_id.name.length")
        guard byteCount > 0, byteCount <= UInt64(Int.max) else {
            throw invalid("fee_payment sponsor program name is invalid")
        }
        let nameBytes = try name.takeBytes(
            Int(byteCount),
            field: "fee_payment.program_id.name"
        )
        guard name.isFinished,
              program.isFinished,
              let value = String(data: nameBytes, encoding: .utf8),
              isCanonicalFeeSponsorProgramName(value) else {
            throw invalid("fee_payment sponsor program name is invalid")
        }
    }

    private static func requireCanonicalAccountController(_ payload: Data) throws {
        var controller = ToriiVerifyingKeyCompactReader(payload)
        let tag = try controller.takeUInt32("fee_payment.program_id.sponsor.controller")
        let body = try controller.takeField("fee_payment.program_id.sponsor.value")
        guard controller.isFinished else {
            throw invalid("fee_payment sponsor account contains trailing bytes")
        }
        switch tag {
        case 0:
            try requireCanonicalPublicKey(body)
        case 1:
            try requireCanonicalMultisigPolicy(body)
        default:
            throw invalid("fee_payment sponsor account controller is unknown")
        }
    }

    private static func requireCanonicalPublicKey(_ payload: Data) throws {
        var key = ToriiVerifyingKeyCompactReader(payload)
        let count = try key.takeUInt64("fee_payment.program_id.sponsor.public_key.count")
        guard count > 1, count <= 8_193 else {
            throw invalid("fee_payment sponsor public key length is invalid")
        }
        var bytes = Data()
        bytes.reserveCapacity(Int(count))
        for _ in 0..<count {
            guard try key.takeLength("fee_payment.program_id.sponsor.public_key.byte") == 1 else {
                throw invalid("fee_payment sponsor public key is not canonical")
            }
            bytes.append(try key.takeUInt8("fee_payment.program_id.sponsor.public_key.byte"))
        }
        guard key.isFinished,
              let algorithmByte = bytes.first,
              let algorithm = SigningAlgorithm(noritoDiscriminant: algorithmByte),
              (try? AccountAddress.fromAccount(
                  publicKey: Data(bytes.dropFirst()),
                  algorithm: algorithm.wireName
              )) != nil else {
            throw invalid("fee_payment sponsor public key is invalid")
        }
    }

    private static func requireCanonicalMultisigPolicy(_ payload: Data) throws {
        var policy = ToriiVerifyingKeyCompactReader(payload)
        var version = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.version")
        )
        guard try version.takeUInt8("fee_payment.program_id.sponsor.multisig.version") == 1,
              version.isFinished else {
            throw invalid("fee_payment sponsor multisig version is invalid")
        }
        var threshold = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.threshold")
        )
        let requiredWeight = try threshold.takeUInt16(
            "fee_payment.program_id.sponsor.multisig.threshold"
        )
        guard requiredWeight > 0, threshold.isFinished else {
            throw invalid("fee_payment sponsor multisig threshold is invalid")
        }
        var members = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.members")
        )
        let memberCount = try members.takeUInt64(
            "fee_payment.program_id.sponsor.multisig.members.count"
        )
        guard memberCount > 0, memberCount <= 1_024 else {
            throw invalid("fee_payment sponsor multisig member count is invalid")
        }
        var totalWeight: UInt64 = 0
        for _ in 0..<memberCount {
            var member = ToriiVerifyingKeyCompactReader(
                try members.takeField("fee_payment.program_id.sponsor.multisig.member")
            )
            try requireCanonicalPublicKey(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.public_key")
            )
            var weight = ToriiVerifyingKeyCompactReader(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.weight")
            )
            let value = try weight.takeUInt16(
                "fee_payment.program_id.sponsor.multisig.member.weight"
            )
            guard value > 0, weight.isFinished, member.isFinished else {
                throw invalid("fee_payment sponsor multisig member is invalid")
            }
            totalWeight += UInt64(value)
        }
        guard members.isFinished,
              policy.isFinished,
              UInt64(requiredWeight) <= totalWeight else {
            throw invalid("fee_payment sponsor multisig policy is invalid")
        }
    }

    private static func requireCanonicalPositiveUInt64(
        _ payload: Data,
        field: String
    ) throws -> UInt64 {
        var value = ToriiVerifyingKeyCompactReader(payload)
        let exact = try value.takeUInt64(field)
        guard exact > 0, value.isFinished else {
            throw invalid("\(field) must be a positive canonical UInt64")
        }
        return exact
    }

    private static func requireCanonicalPositiveUInt64Option(
        _ payload: Data,
        field: String
    ) throws -> UInt64? {
        var option = ToriiVerifyingKeyCompactReader(payload)
        switch try option.takeUInt8(field) {
        case 0:
            guard option.isFinished else {
                throw invalid("\(field) None encoding contains trailing bytes")
            }
            return nil
        case 1:
            let exact = try requireCanonicalPositiveUInt64(
                try option.takeField(field),
                field: field
            )
            guard option.isFinished else {
                throw invalid("\(field) Some encoding contains trailing bytes")
            }
            return exact
        default:
            throw invalid("\(field) contains an invalid option tag")
        }
    }

    private static func invalid(_ message: String) -> ToriiClientError {
        .invalidPayload(message)
    }
}

private extension ToriiVerifyingKeyCompactReader {
    mutating func takeUInt16(_ field: String) throws -> UInt16 {
        let bytes = try takeBytes(MemoryLayout<UInt16>.size, field: field)
        return bytes.withUnsafeBytes {
            UInt16(littleEndian: $0.loadUnaligned(as: UInt16.self))
        }
    }
}
