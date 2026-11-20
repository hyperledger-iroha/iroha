import Foundation

/// Structural checks for the `FeePaymentIntent` and metadata fields of a
/// canonical `TransactionPayload`.
///
/// Every check reads the payload in one named layout: `.compact`, the layout of
/// the current transaction wire, unless the caller names another. The layout is
/// never inferred from the bytes.
enum TransactionFeePaymentValidation {
    /// Validates the canonical `FeePaymentIntent` shared by transaction payloads.
    static func requireCanonicalTransactionFeePayment(
        _ payload: Data,
        layout: NoritoWireLayout = .compact
    ) throws {
        var intent = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        let payer = try intent.takeEnumTag("fee_payment.payer")
        var value = ToriiVerifyingKeyCompactReader(
            try intent.takeField("fee_payment.value"),
            layout: layout
        )
        switch payer {
        case 0:
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits"),
                layout: layout
            )
            _ = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit",
                layout: layout
            )
        case 1:
            try requireCanonicalSponsorProgram(
                try value.takeField("fee_payment.program_id"),
                layout: layout
            )
            _ = try requireCanonicalPositiveUInt64(
                try value.takeField("fee_payment.program_revision"),
                field: "fee_payment.program_revision",
                layout: layout
            )
            try requireCanonicalChargeLimits(
                try value.takeField("fee_payment.charge_limits"),
                layout: layout
            )
            _ = try requireCanonicalPositiveUInt64Option(
                try value.takeField("fee_payment.gas_limit"),
                field: "fee_payment.gas_limit",
                layout: layout
            )
        default:
            throw invalid("fee_payment contains an unknown payer variant")
        }
        guard value.isFinished, intent.isFinished else {
            throw invalid("fee_payment contains trailing bytes")
        }
    }

    /// Requires the exact empty metadata encoding used when a request has no metadata.
    static func requireEmptyTransactionMetadata(
        _ payload: Data,
        layout: NoritoWireLayout = .compact
    ) throws {
        var metadata = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        guard try metadata.takeCount("metadata.count") == 0,
              metadata.isFinished else {
            throw invalid("transaction metadata must use the exact empty encoding")
        }
    }

    private static func requireCanonicalChargeLimits(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var limits = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        // Norito's compact-length flag applies to enclosing fields and
        // elements, while sequence counts remain fixed-width little-endian.
        // The dense layout writes the count as a varint.
        let count = try limits.takeCount("fee_payment.charge_limits.count")
        guard count <= UInt64(FeeChargeKind.allCases.count) else {
            throw invalid("fee_payment.charge_limits contains duplicate or unknown charge kinds")
        }
        var previousKind: UInt32?
        for _ in 0..<count {
            var limit = ToriiVerifyingKeyCompactReader(
                try limits.takeField("fee_payment.charge_limits.item"),
                layout: layout
            )
            var kind = ToriiVerifyingKeyCompactReader(
                try limit.takeField("fee_payment.charge_limits.kind"),
                layout: layout
            )
            let rawKind = try kind.takeEnumTag("fee_payment.charge_limits.kind")
            guard kind.isFinished, FeeChargeKind(rawValue: rawKind) != nil,
                  previousKind.map({ $0 < rawKind }) ?? true else {
                throw invalid("fee_payment.charge_limits must use unique canonical charge-kind order")
            }
            previousKind = rawKind
            try requireCanonicalAssetDefinition(
                try limit.takeField("fee_payment.charge_limits.asset_definition_id"),
                layout: layout
            )
            try requireCanonicalPositiveQuantity(
                try limit.takeField("fee_payment.charge_limits.max_amount"),
                layout: layout
            )
            guard limit.isFinished else {
                throw invalid("fee_payment.charge_limits item contains trailing bytes")
            }
        }
        guard limits.isFinished else {
            throw invalid("fee_payment.charge_limits contains trailing bytes")
        }
    }

    private static func requireCanonicalAssetDefinition(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var asset = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        let bytes: Data
        do {
            bytes = try asset.takeByteElements(16, field: "fee_payment.asset_definition_id.byte")
        } catch {
            throw invalid("fee_payment asset definition must use the exact 16-byte encoding")
        }
        guard asset.isFinished, AssetDefinitionAddress.encode(uuidBytes: bytes) != nil else {
            throw invalid("fee_payment asset definition must use the exact 16-byte encoding")
        }
    }

    private static func requireCanonicalPositiveQuantity(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var quantity = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        var mantissa = ToriiVerifyingKeyCompactReader(
            try quantity.takeField("fee_payment.max_amount.mantissa"),
            layout: layout
        )
        // The `BigInt` byte count is a fixed `u32` in every layout (`norito.md`).
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
            try quantity.takeField("fee_payment.max_amount.scale"),
            layout: layout
        )
        guard try scale.takeUInt32("fee_payment.max_amount.scale") <= CanonicalNorito.maxNumericScale,
              scale.isFinished,
              quantity.isFinished else {
            throw invalid("fee_payment maximum contains an invalid numeric scale")
        }
    }

    private static func requireCanonicalSponsorProgram(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var program = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        try requireCanonicalAccountController(
            try program.takeField("fee_payment.program_id.sponsor"),
            layout: layout
        )
        var name = ToriiVerifyingKeyCompactReader(
            try program.takeField("fee_payment.program_id.name"),
            layout: layout
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

    private static func requireCanonicalAccountController(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var controller = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        let tag = try controller.takeEnumTag("fee_payment.program_id.sponsor.controller")
        let body = try controller.takeField("fee_payment.program_id.sponsor.value")
        guard controller.isFinished else {
            throw invalid("fee_payment sponsor account contains trailing bytes")
        }
        switch tag {
        case 0:
            try requireCanonicalPublicKey(body, layout: layout)
        case 1:
            try requireCanonicalMultisigPolicy(body, layout: layout)
        default:
            throw invalid("fee_payment sponsor account controller is unknown")
        }
    }

    private static func requireCanonicalPublicKey(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var key = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        let count = try key.takeCount("fee_payment.program_id.sponsor.public_key.count")
        guard count > 1, count <= 8_193 else {
            throw invalid("fee_payment sponsor public key length is invalid")
        }
        let bytes: Data
        do {
            bytes = try key.takeByteElements(
                Int(count),
                field: "fee_payment.program_id.sponsor.public_key.byte"
            )
        } catch {
            throw invalid("fee_payment sponsor public key is not canonical")
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

    private static func requireCanonicalMultisigPolicy(
        _ payload: Data,
        layout: NoritoWireLayout
    ) throws {
        var policy = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        var version = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.version"),
            layout: layout
        )
        guard try version.takeUInt8("fee_payment.program_id.sponsor.multisig.version") == 1,
              version.isFinished else {
            throw invalid("fee_payment sponsor multisig version is invalid")
        }
        var threshold = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.threshold"),
            layout: layout
        )
        let requiredWeight = try threshold.takeUInt16(
            "fee_payment.program_id.sponsor.multisig.threshold"
        )
        guard requiredWeight > 0, threshold.isFinished else {
            throw invalid("fee_payment sponsor multisig threshold is invalid")
        }
        var members = ToriiVerifyingKeyCompactReader(
            try policy.takeField("fee_payment.program_id.sponsor.multisig.members"),
            layout: layout
        )
        let memberCount = try members.takeCount(
            "fee_payment.program_id.sponsor.multisig.members.count"
        )
        guard memberCount > 0, memberCount <= 1_024 else {
            throw invalid("fee_payment sponsor multisig member count is invalid")
        }
        var totalWeight: UInt64 = 0
        for _ in 0..<memberCount {
            var member = ToriiVerifyingKeyCompactReader(
                try members.takeField("fee_payment.program_id.sponsor.multisig.member"),
                layout: layout
            )
            try requireCanonicalPublicKey(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.public_key"),
                layout: layout
            )
            var weight = ToriiVerifyingKeyCompactReader(
                try member.takeField("fee_payment.program_id.sponsor.multisig.member.weight"),
                layout: layout
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
        field: String,
        layout: NoritoWireLayout
    ) throws -> UInt64 {
        var value = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        let exact = try value.takeUInt64(field)
        guard exact > 0, value.isFinished else {
            throw invalid("\(field) must be a positive canonical UInt64")
        }
        return exact
    }

    private static func requireCanonicalPositiveUInt64Option(
        _ payload: Data,
        field: String,
        layout: NoritoWireLayout
    ) throws -> UInt64? {
        var option = ToriiVerifyingKeyCompactReader(payload, layout: layout)
        switch try option.takeUInt8(field) {
        case 0:
            guard option.isFinished else {
                throw invalid("\(field) None encoding contains trailing bytes")
            }
            return nil
        case 1:
            let exact = try requireCanonicalPositiveUInt64(
                try option.takeField(field),
                field: field,
                layout: layout
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
