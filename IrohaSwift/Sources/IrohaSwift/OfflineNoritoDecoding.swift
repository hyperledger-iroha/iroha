import Foundation

enum OfflineNoritoDecodingError: Error, LocalizedError, Sendable {
    case truncatedPayload
    case invalidField(String)

    var errorDescription: String? {
        switch self {
        case .truncatedPayload:
            return "Norito payload ended unexpectedly."
        case .invalidField(let reason):
            return "Invalid Norito field: \(reason)"
        }
    }
}

struct OfflineNoritoReader {
    private let data: Data
    private(set) var offset: Int = 0

    init(data: Data) {
        self.data = data
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    mutating func readUInt16LE() throws -> UInt16 {
        let bytes = try readBytes(2)
        var value: UInt16 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 2)
        }
        return UInt16(littleEndian: value)
    }

    mutating func readUInt32LE() throws -> UInt32 {
        let bytes = try readBytes(4)
        var value: UInt32 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 4)
        }
        return UInt32(littleEndian: value)
    }

    mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard offset + count <= data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let start = data.startIndex + offset
        let result = Data(data[start..<(start + count)])
        offset += count
        return result
    }

    mutating func readVarint() throws -> UInt64 {
        var shift: UInt64 = 0
        var value: UInt64 = 0
        while true {
            let byte = try readUInt8()
            guard shift < 64 else {
                throw OfflineNoritoDecodingError.invalidField("varint length overflow")
            }
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                return value
            }
            shift += 7
        }
    }

    mutating func readField() throws -> Data {
        let length = try readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    mutating func readCompactField() throws -> Data {
        let length = try readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    func remaining() -> Int {
        data.count - offset
    }
}

struct ParsedPublicAssetLiteral {
    let assetDefinitionId: String
    let accountId: String
    let dataspaceId: UInt64?
}

public enum OfflineNoteV2Decoding {
    public static func decodeCertificatePayload(_ data: Data) throws -> OfflineNoteKeyCertificatePayloadV2 {
        try OfflineNoteDecoding.decodeKeyCertificatePayloadV2(data)
    }

    public static func decodeKeyCertificatePayload(_ data: Data) throws -> OfflineNoteKeyCertificatePayloadV2 {
        try decodeCertificatePayload(data)
    }

    public static func decodeCertificate(_ data: Data) throws -> OfflineNoteKeyCertificateV2 {
        try OfflineNoteDecoding.decodeKeyCertificateV2(data)
    }

    public static func decodeIssue(_ data: Data) throws -> OfflineNoteIssueV2 {
        try OfflineNoteDecoding.decodeIssueV2(data)
    }

    public static func decodeIssuedClaim(_ data: Data) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteDecoding.decodeIssuedClaimV2(data)
    }

    public static func decodeAuditOutputClaim(_ data: Data) throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteDecoding.decodeAuditOutputClaimV2(data)
    }

    public static func decodeRecursiveProof(_ data: Data) throws -> OfflineNoteRecursiveProofV2 {
        try OfflineNoteDecoding.decodeRecursiveProofV2(data)
    }

    public static func decodeRedeem(_ data: Data) throws -> OfflineNoteRedeemV2 {
        try OfflineNoteDecoding.decodeRedeemV2(data)
    }

    public static func decodeRedeemPublicInputs(_ data: Data) throws -> OfflineNoteRedeemPublicInputsV2 {
        try OfflineNoteDecoding.decodeRedeemPublicInputsV2(data)
    }

    public static func decodeAudit(_ data: Data) throws -> OfflineNoteAuditBundleV2 {
        try OfflineNoteDecoding.decodeAuditV2(data)
    }

    public static func decodeAuditPublicInputs(_ data: Data) throws -> OfflineNoteAuditPublicInputsV2 {
        try OfflineNoteDecoding.decodeAuditPublicInputsV2(data)
    }

    public static func decodeIssueInstruction(_ data: Data) throws -> OfflineNoteIssueV2 {
        try OfflineNoteDecoding.decodeIssueInstructionV2(data)
    }

    public static func decodeRedeemInstruction(_ data: Data) throws -> OfflineNoteRedeemV2 {
        try OfflineNoteDecoding.decodeRedeemInstructionV2(data)
    }

    public static func decodeAuditInstruction(_ data: Data) throws -> OfflineNoteAuditBundleV2 {
        try OfflineNoteDecoding.decodeAuditInstructionV2(data)
    }
}

public enum OfflineNoteDecoding {
    public static func decodeKeyCertificatePayload(_ data: Data) throws -> OfflineNoteKeyCertificatePayload {
        try decodePayload(data, typeName: OfflineNoteTypeNames.keyCertificatePayload) { reader in
            try OfflineNoteKeyCertificatePayload(
                domain: readField(&reader, readString),
                version: readField(&reader) { try $0.readUInt16LE() },
                platform: readField(&reader, readString),
                keyId: readField(&reader, readString),
                deviceId: readField(&reader, readString),
                accountId: readField(&reader, readAccountId),
                publicKey: readField(&reader, readBytesVec),
                assertionScheme: readField(&reader, readString),
                assertionKeyAlgorithm: readField(&reader, readString),
                assertionPublicKey: readField(&reader, readBytesVec),
                assertionUsageCountLimit: readField(&reader, readOptionUInt32),
                oneUse: readField(&reader, readBool)
            )
        }
    }

    public static func decodeKeyCertificate(_ data: Data) throws -> OfflineNoteKeyCertificate {
        try decodePayload(data, typeName: OfflineNoteTypeNames.keyCertificate) { reader in
            try decodeKeyCertificatePayloadFields(&reader, includesDomain: false)
        }
    }

    public static func decodeIssue(_ data: Data) throws -> OfflineNoteIssue {
        try decodePayload(data, typeName: OfflineNoteTypeNames.issue, decode: decodeIssueFields)
    }

    public static func decodeIssuedClaim(_ data: Data) throws -> OfflineNoteIssuedClaim {
        try decodePayload(data, typeName: OfflineNoteTypeNames.issuedClaim) { reader in
            try decodeIssuedClaimFields(&reader)
        }
    }

    public static func decodeRedeem(_ data: Data) throws -> OfflineNoteRedeem {
        try decodePayload(data, typeName: OfflineNoteTypeNames.redeem, decode: decodeRedeemFields)
    }

    public static func decodeRedeemPublicInputs(_ data: Data) throws -> OfflineNoteRedeemPublicInputs {
        try decodePayload(data, typeName: OfflineNoteTypeNames.redeemPublicInputs) { reader in
            try OfflineNoteRedeemPublicInputs(
                domain: readField(&reader, readString),
                sourceNoteCommitment: readField(&reader) { child in try readHash(&child, field: "source_note_commitment") },
                inputNullifiers: readField(&reader) { child in
                    try readVec(&child) { element in try readHash(&element, field: "input_nullifier") }
                },
                keyCertificatePayloadHash: readField(&reader) { child in try readHash(&child, field: "key_certificate_payload_hash") },
                recipient: readField(&reader, readAccountId),
                assetId: readField(&reader, readAssetId),
                amount: readField(&reader, readNumeric)
            )
        }
    }

    public static func decodeAudit(_ data: Data) throws -> OfflineNoteAuditBundle {
        try decodePayload(data, typeName: OfflineNoteTypeNames.audit, decode: decodeAuditFields)
    }

    public static func decodeAuditPublicInputs(_ data: Data) throws -> OfflineNoteAuditPublicInputs {
        try decodePayload(data, typeName: OfflineNoteTypeNames.auditPublicInputs) { reader in
            try OfflineNoteAuditPublicInputs(
                domain: readField(&reader, readString),
                tokenId: readField(&reader) { child in try readHash(&child, field: "token_id") },
                keyCertificatePayloadHash: readField(&reader) { child in try readHash(&child, field: "key_certificate_payload_hash") },
                inputNullifiers: readField(&reader) { child in
                    try readVec(&child) { element in try readHash(&element, field: "input_nullifier") }
                },
                inputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimFields) },
                outputCommitments: readField(&reader) { child in
                    try readVec(&child) { element in try readHash(&element, field: "output_commitment") }
                },
                outputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimFields) }
            )
        }
    }

    public static func decodeKeyCertificatePayloadV2(_ data: Data) throws -> OfflineNoteKeyCertificatePayloadV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.keyCertificatePayload) { reader in
            try decodeKeyCertificatePayloadV2Fields(&reader)
        }
    }

    public static func decodeKeyCertificateV2(_ data: Data) throws -> OfflineNoteKeyCertificateV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.keyCertificate) { reader in
            try decodeKeyCertificateV2Fields(&reader)
        }
    }

    public static func decodeIssueV2(_ data: Data) throws -> OfflineNoteIssueV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.issue, decode: decodeIssueV2Fields)
    }

    public static func decodeIssuedClaimV2(_ data: Data) throws -> OfflineNoteIssuedClaimV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.issuedClaim, decode: decodeIssuedClaimV2Fields)
    }

    public static func decodeAuditOutputClaimV2(_ data: Data) throws -> OfflineNoteAuditOutputClaimV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.auditOutputClaim, decode: decodeAuditOutputClaimV2Fields)
    }

    public static func decodeRecursiveProofV2(_ data: Data) throws -> OfflineNoteRecursiveProofV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.recursiveProof, decode: decodeRecursiveProofV2Fields)
    }

    public static func decodeRedeemV2(_ data: Data) throws -> OfflineNoteRedeemV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.redeem, decode: decodeRedeemV2Fields)
    }

    public static func decodeRedeemPublicInputsV2(_ data: Data) throws -> OfflineNoteRedeemPublicInputsV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.redeemPublicInputs) { reader in
            try OfflineNoteRedeemPublicInputsV2(
                domain: readField(&reader, readString),
                sourceNoteCommitment: readField(&reader) { child in
                    try readHashV2(&child, field: "source_note_commitment")
                },
                inputNullifiers: readField(&reader) { child in
                    try readVec(&child) { element in try readHashV2(&element, field: "input_nullifier") }
                },
                keyCertificatePayloadHash: readField(&reader) { child in
                    try readHashV2(&child, field: "key_certificate_payload_hash")
                },
                recipient: readField(&reader, readAccountId),
                assetId: readField(&reader, readAssetId),
                amount: readField(&reader, readNumeric)
            )
        }
    }

    public static func decodeAuditV2(_ data: Data) throws -> OfflineNoteAuditBundleV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.audit, decode: decodeAuditV2Fields)
    }

    public static func decodeAuditPublicInputsV2(_ data: Data) throws -> OfflineNoteAuditPublicInputsV2 {
        try decodePayload(data, typeName: OfflineNoteV2TypeNames.auditPublicInputs) { reader in
            try OfflineNoteAuditPublicInputsV2(
                domain: readField(&reader, readString),
                tokenId: readField(&reader) { child in try readHashV2(&child, field: "token_id") },
                keyCertificatePayloadHash: readField(&reader) { child in
                    try readHashV2(&child, field: "key_certificate_payload_hash")
                },
                inputNullifiers: readField(&reader) { child in
                    try readVec(&child) { element in try readHashV2(&element, field: "input_nullifier") }
                },
                inputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimV2Fields) },
                outputCommitments: readField(&reader) { child in
                    try readVec(&child) { element in try readHashV2(&element, field: "output_commitment") }
                },
                outputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimV2Fields) }
            )
        }
    }

    public static func decodeIssueInstructionV2(_ data: Data) throws -> OfflineNoteIssueV2 {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteV2TypeNames.issueInstruction,
            compatibleInstructionTypeNames: [OfflineNoteV2TypeNames.issueInstructionAlias],
            decodeHeader: decodeIssueV2,
            decodeBare: decodeIssueV2Fields
        )
    }

    public static func decodeRedeemInstructionV2(_ data: Data) throws -> OfflineNoteRedeemV2 {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteV2TypeNames.redeemInstruction,
            compatibleInstructionTypeNames: [OfflineNoteV2TypeNames.redeemInstructionAlias],
            decodeHeader: decodeRedeemV2,
            decodeBare: decodeRedeemV2Fields
        )
    }

    public static func decodeAuditInstructionV2(_ data: Data) throws -> OfflineNoteAuditBundleV2 {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteV2TypeNames.auditInstruction,
            compatibleInstructionTypeNames: [OfflineNoteV2TypeNames.auditInstructionAlias],
            decodeHeader: decodeAuditV2,
            decodeBare: decodeAuditV2Fields
        )
    }

    public static func decodeNoteCommitmentPreimage(_ data: Data) throws -> OfflineNoteCommitmentPreimage {
        try decodePayload(data, typeName: OfflineNoteTypeNames.noteCommitmentPreimage) { reader in
            try OfflineNoteCommitmentPreimage(
                domain: readField(&reader, readString),
                chainId: readField(&reader, readChainId),
                ownerKeyCertificatePayloadHash: readField(&reader) { child in
                    try readHash(&child, field: "owner_key_certificate_payload_hash")
                },
                assetId: readField(&reader, readAssetId),
                amount: readField(&reader, readNumeric),
                noteSecret: readField(&reader, readBytesVec),
                origin: readField(&reader, readCommitmentOrigin)
            )
        }
    }

    public static func decodeInputNullifierPreimage(_ data: Data) throws -> OfflineNoteInputNullifierPreimage {
        try decodePayload(data, typeName: OfflineNoteTypeNames.inputNullifierPreimage) { reader in
            try OfflineNoteInputNullifierPreimage(
                domain: readField(&reader, readString),
                chainId: readField(&reader, readChainId),
                sourceNoteCommitment: readField(&reader) { child in try readHash(&child, field: "source_note_commitment") },
                ownerKeyCertificatePayloadHash: readField(&reader) { child in
                    try readHash(&child, field: "owner_key_certificate_payload_hash")
                },
                noteSecret: readField(&reader, readBytesVec)
            )
        }
    }

    public static func decodePaymentTokenIdPreimage(_ data: Data) throws -> OfflineNotePaymentTokenIdPreimage {
        try decodePayload(data, typeName: OfflineNoteTypeNames.paymentTokenIdPreimage) { reader in
            try OfflineNotePaymentTokenIdPreimage(
                domain: readField(&reader, readString),
                chainId: readField(&reader, readChainId),
                paymentRequestId: readField(&reader, readString),
                createdAtMs: readField(&reader) { try $0.readUInt64LE() },
                tokenNonce: readField(&reader, readBytesVec),
                senderKeyCertificatePayloadHash: readField(&reader) { child in
                    try readHash(&child, field: "sender_key_certificate_payload_hash")
                },
                inputNullifiers: readField(&reader) { child in
                    try readVec(&child) { element in try readHash(&element, field: "input_nullifier") }
                },
                outputCommitments: readField(&reader) { child in
                    try readVec(&child) { element in try readHash(&element, field: "output_commitment") }
                }
            )
        }
    }

    public static func decodeIssueInstruction(_ data: Data) throws -> OfflineNoteIssue {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteTypeNames.issueInstruction,
            decodeHeader: decodeIssue,
            decodeBare: decodeIssueFields
        )
    }

    public static func decodeRedeemInstruction(_ data: Data) throws -> OfflineNoteRedeem {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteTypeNames.redeemInstruction,
            decodeHeader: decodeRedeem,
            decodeBare: decodeRedeemFields
        )
    }

    public static func decodeAuditInstruction(_ data: Data) throws -> OfflineNoteAuditBundle {
        try decodeInstructionModel(
            data,
            instructionTypeName: OfflineNoteTypeNames.auditInstruction,
            decodeHeader: decodeAudit,
            decodeBare: decodeAuditFields
        )
    }

    private static func decodePayload<T>(
        _ data: Data,
        typeName: String,
        decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        guard let frame = noritoDecodeFrame(data) else {
            throw OfflineNoritoDecodingError.invalidField("invalid Norito frame")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: typeName) else {
            throw OfflineNoritoDecodingError.invalidField("schema hash mismatch")
        }
        guard frame.header.compression == .none else {
            throw OfflineNoritoDecodingError.invalidField("compressed payloads are not supported")
        }
        guard (frame.header.flags & NoritoHeader.compactLen) != 0 else {
            throw OfflineNoritoDecodingError.invalidField("Offline Note payload must use compact lengths")
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let value = try decode(&reader)
        try requireFullyRead(reader)
        return value
    }

    private static func decodeInstructionModel<T>(
        _ data: Data,
        instructionTypeName: String,
        compatibleInstructionTypeNames: [String] = [],
        decodeHeader: (Data) throws -> T,
        decodeBare: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        let instructionTypeNames = [instructionTypeName] + compatibleInstructionTypeNames
        let wirePayload = try extractInstructionWirePayload(data, expectedWireNames: instructionTypeNames)
        let modelPayload = try decodeInstructionWrapper(wirePayload, typeNames: instructionTypeNames)
        if isNoritoFrame(modelPayload) {
            return try decodeHeader(modelPayload)
        }
        var reader = OfflineNoritoReader(data: modelPayload)
        let value = try decodeBare(&reader)
        try requireFullyRead(reader)
        return value
    }

    private static func extractInstructionWirePayload(_ data: Data, expectedWireNames: [String]) throws -> Data {
        if isNoritoFrame(data) {
            return data
        }
        if let payload = tryDecodeInstructionPair(data, expectedWireNames: expectedWireNames, compact: true) {
            return payload
        }
        if let payload = tryDecodeInstructionPair(data, expectedWireNames: expectedWireNames, compact: false) {
            return payload
        }
        throw OfflineNoritoDecodingError.invalidField("invalid instruction envelope")
    }

    private static func tryDecodeInstructionPair(
        _ data: Data,
        expectedWireNames: [String],
        compact: Bool
    ) -> Data? {
        do {
            var reader = OfflineNoritoReader(data: data)
            let wireName = try readFramedField(&reader, compact: compact) { child in
                try readString(&child, compact: compact)
            }
            guard expectedWireNames.contains(wireName) else { return nil }
            let payload = try readFramedField(&reader, compact: compact) { child in
                try readBytesVec(&child)
            }
            try requireFullyRead(reader)
            return payload
        } catch {
            return nil
        }
    }

    private static func decodeInstructionWrapper(_ data: Data, typeNames: [String]) throws -> Data {
        guard let frame = noritoDecodeFrame(data) else {
            throw OfflineNoritoDecodingError.invalidField("invalid instruction Norito frame")
        }
        guard typeNames.contains(where: { frame.header.schema == noritoSchemaHash(forTypeName: $0) }) else {
            throw OfflineNoritoDecodingError.invalidField("instruction schema hash mismatch")
        }
        guard frame.header.compression == .none else {
            throw OfflineNoritoDecodingError.invalidField("compressed instruction payloads are not supported")
        }
        let compact = (frame.header.flags & NoritoHeader.compactLen) != 0
        var reader = OfflineNoritoReader(data: frame.payload)
        let modelPayload = try readFramedField(&reader, compact: compact) { child in
            try child.readBytes(child.remaining())
        }
        try requireFullyRead(reader)
        return modelPayload
    }

    private static func isNoritoFrame(_ data: Data) -> Bool {
        data.count >= NoritoHeader.encodedLength && data.prefix(4) == NoritoHeader.magic
    }

    private static func decodeKeyCertificatePayloadFields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteKeyCertificate {
        try decodeKeyCertificatePayloadFields(&reader, includesDomain: false)
    }

    private static func decodeKeyCertificatePayloadFields(
        _ reader: inout OfflineNoritoReader,
        includesDomain: Bool
    ) throws -> OfflineNoteKeyCertificate {
        if includesDomain {
            _ = try readField(&reader, readString)
        }
        return try OfflineNoteKeyCertificate(
            version: readField(&reader) { try $0.readUInt16LE() },
            platform: readField(&reader, readString),
            keyId: readField(&reader, readString),
            deviceId: readField(&reader, readString),
            accountId: readField(&reader, readAccountId),
            publicKey: readField(&reader, readBytesVec),
            assertionScheme: readField(&reader, readString),
            assertionKeyAlgorithm: readField(&reader, readString),
            assertionPublicKey: readField(&reader, readBytesVec),
            assertionUsageCountLimit: readField(&reader, readOptionUInt32),
            oneUse: readField(&reader, readBool),
            issuerSignature: readField(&reader, readConstVec)
        )
    }

    private static func decodeIssueFields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteIssue {
        try OfflineNoteIssue(
            noteCommitment: readField(&reader) { child in try readHash(&child, field: "note_commitment") },
            keyCertificate: readField(&reader, decodeKeyCertificatePayloadFields),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeIssuedClaimFields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            domain: readField(&reader, readString),
            noteCommitment: readField(&reader) { child in try readHash(&child, field: "note_commitment") },
            keyCertificatePayloadHash: readField(&reader) { child in
                try readHash(&child, field: "key_certificate_payload_hash")
            },
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeAuditOutputClaimFields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteAuditOutputClaim {
        try OfflineNoteAuditOutputClaim(
            noteCommitment: readField(&reader) { child in try readHash(&child, field: "note_commitment") },
            keyCertificate: readField(&reader, decodeKeyCertificatePayloadFields),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeRecursiveProofFields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteRecursiveProof {
        try OfflineNoteRecursiveProof(
            verifierKeyId: readField(&reader, readVerifyingKeyId),
            publicInputsHash: readField(&reader) { child in try readHash(&child, field: "public_inputs_hash") },
            proof: readField(&reader, readProofBox)
        )
    }

    private static func decodeRedeemFields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteRedeem {
        try OfflineNoteRedeem(
            sourceNoteCommitment: readField(&reader) { child in try readHash(&child, field: "source_note_commitment") },
            inputNullifiers: readField(&reader) { child in
                try readVec(&child) { element in try readHash(&element, field: "input_nullifier") }
            },
            senderKeyCertificate: readField(&reader, decodeKeyCertificatePayloadFields),
            recipient: readField(&reader, readAccountId),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric),
            recursiveProof: readField(&reader, decodeRecursiveProofFields)
        )
    }

    private static func decodeAuditFields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteAuditBundle {
        try OfflineNoteAuditBundle(
            tokenId: readField(&reader) { child in try readHash(&child, field: "token_id") },
            senderKeyCertificate: readField(&reader, decodeKeyCertificatePayloadFields),
            inputNullifiers: readField(&reader) { child in
                try readVec(&child) { element in try readHash(&element, field: "input_nullifier") }
            },
            inputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimFields) },
            outputCommitments: readField(&reader) { child in
                try readVec(&child) { element in try readHash(&element, field: "output_commitment") }
            },
            outputClaims: readField(&reader) { child in try readVec(&child, decodeAuditOutputClaimFields) },
            recursiveProof: readField(&reader, decodeRecursiveProofFields)
        )
    }

    private static func decodeKeyCertificatePayloadV2Fields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteKeyCertificatePayloadV2 {
        try OfflineNoteKeyCertificatePayloadV2(
            domain: readField(&reader, readString),
            version: readField(&reader) { try $0.readUInt16LE() },
            platform: readField(&reader, readString),
            keyId: readField(&reader, readString),
            deviceId: readField(&reader, readString),
            accountId: readField(&reader, readAccountId),
            publicKey: readField(&reader, readBytesVec),
            assertionScheme: readField(&reader, readString),
            assertionKeyAlgorithm: readField(&reader, readString),
            assertionPublicKey: readField(&reader, readBytesVec),
            assertionUsageCountLimit: readField(&reader, readOptionUInt32),
            oneUse: readField(&reader, readBool)
        )
    }

    private static func decodeKeyCertificateV2Fields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteKeyCertificateV2 {
        try OfflineNoteKeyCertificateV2(
            version: readField(&reader) { try $0.readUInt16LE() },
            platform: readField(&reader, readString),
            keyId: readField(&reader, readString),
            deviceId: readField(&reader, readString),
            accountId: readField(&reader, readAccountId),
            publicKey: readField(&reader, readBytesVec),
            assertionScheme: readField(&reader, readString),
            assertionKeyAlgorithm: readField(&reader, readString),
            assertionPublicKey: readField(&reader, readBytesVec),
            assertionUsageCountLimit: readField(&reader, readOptionUInt32),
            oneUse: readField(&reader, readBool),
            issuerSignature: readField(&reader, readConstVec)
        )
    }

    private static func decodeIssueV2Fields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteIssueV2 {
        try OfflineNoteIssueV2(
            noteCommitment: readField(&reader) { child in try readHashV2(&child, field: "note_commitment") },
            keyCertificate: readField(&reader, decodeKeyCertificateV2Fields),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeIssuedClaimV2Fields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            domain: readField(&reader, readString),
            noteCommitment: readField(&reader) { child in try readHashV2(&child, field: "note_commitment") },
            keyCertificatePayloadHash: readField(&reader) { child in
                try readHashV2(&child, field: "key_certificate_payload_hash")
            },
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeAuditOutputClaimV2Fields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteAuditOutputClaimV2(
            noteCommitment: readField(&reader) { child in try readHashV2(&child, field: "note_commitment") },
            keyCertificate: readField(&reader, decodeKeyCertificateV2Fields),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric)
        )
    }

    private static func decodeRecursiveProofV2Fields(
        _ reader: inout OfflineNoritoReader
    ) throws -> OfflineNoteRecursiveProofV2 {
        try OfflineNoteRecursiveProofV2(
            verifierKeyId: readField(&reader, readVerifyingKeyId),
            publicInputsHash: readField(&reader) { child in try readHashV2(&child, field: "public_inputs_hash") },
            proof: readField(&reader, readProofBoxV2)
        )
    }

    private static func decodeRedeemV2Fields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteRedeemV2 {
        try OfflineNoteRedeemV2(
            sourceNoteCommitment: readField(&reader) { child in try readHashV2(&child, field: "source_note_commitment") },
            inputNullifiers: readField(&reader) { child in
                try readVec(&child) { element in try readHashV2(&element, field: "input_nullifier") }
            },
            senderKeyCertificate: readField(&reader, decodeKeyCertificateV2Fields),
            recipient: readField(&reader, readAccountId),
            assetId: readField(&reader, readAssetId),
            amount: readField(&reader, readNumeric),
            recursiveProof: readField(&reader, decodeRecursiveProofV2Fields)
        )
    }

    private static func decodeAuditV2Fields(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteAuditBundleV2 {
        try OfflineNoteAuditBundleV2(
            tokenId: readField(&reader) { child in try readHashV2(&child, field: "token_id") },
            senderKeyCertificate: readField(&reader, decodeKeyCertificateV2Fields),
            inputNullifiers: readField(&reader) { child in
                try readVec(&child) { element in try readHashV2(&element, field: "input_nullifier") }
            },
            inputClaims: readField(&reader) { child in try readVec(&child, decodeIssuedClaimV2Fields) },
            outputCommitments: readField(&reader) { child in
                try readVec(&child) { element in try readHashV2(&element, field: "output_commitment") }
            },
            outputClaims: readField(&reader) { child in try readVec(&child, decodeAuditOutputClaimV2Fields) },
            recursiveProof: readField(&reader, decodeRecursiveProofV2Fields)
        )
    }

    private static func readField<T>(
        _ reader: inout OfflineNoritoReader,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        var child = OfflineNoritoReader(data: try reader.readCompactField())
        let value = try decode(&child)
        try requireFullyRead(child)
        return value
    }

    private static func readFramedField<T>(
        _ reader: inout OfflineNoritoReader,
        compact: Bool,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        let payload = compact ? try reader.readCompactField() : try reader.readField()
        var child = OfflineNoritoReader(data: payload)
        let value = try decode(&child)
        try requireFullyRead(child)
        return value
    }

    private static func readVec<T>(
        _ reader: inout OfflineNoritoReader,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> [T] {
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("vector length overflow")
        }
        var values: [T] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<Int(count) {
            values.append(try readField(&reader, decode))
        }
        return values
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        try readString(&reader, compact: true)
    }

    private static func readString(_ reader: inout OfflineNoritoReader, compact: Bool) throws -> String {
        let length = compact ? try reader.readVarint() : try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8")
        }
        return value
    }

    private static func readBool(_ reader: inout OfflineNoritoReader) throws -> Bool {
        switch try reader.readUInt8() {
        case 0: return false
        case 1: return true
        default: throw OfflineNoritoDecodingError.invalidField("invalid boolean tag")
        }
    }

    private static func readBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("byte vector length overflow")
        }
        return try reader.readBytes(Int(length))
    }

    private static func readConstVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("const vector length overflow")
        }
        var out = Data()
        out.reserveCapacity(Int(count))
        for _ in 0..<Int(count) {
            let length = try reader.readVarint()
            guard length == 1 else {
                throw OfflineNoritoDecodingError.invalidField("const u8 element length must be 1")
            }
            out.append(try reader.readUInt8())
        }
        return out
    }

    private static func readOptionUInt32(_ reader: inout OfflineNoritoReader) throws -> UInt32? {
        switch try reader.readUInt8() {
        case 0: return nil
        case 1: return try readField(&reader) { try $0.readUInt32LE() }
        default: throw OfflineNoritoDecodingError.invalidField("invalid option tag")
        }
    }

    private static func readHash(_ reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        try OfflineNoteValidation.validateHash(bytes, field: field)
        return bytes
    }

    private static func readHashV2(_ reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        try OfflineNoteV2Validation.validateHash(bytes, field: field)
        return bytes
    }

    private static func readVerifyingKeyId(_ reader: inout OfflineNoritoReader) throws -> VerifyingKeyIdReference {
        try VerifyingKeyIdReference(
            backend: readField(&reader, readString),
            name: readField(&reader, readString)
        )
    }

    private static func readProofBox(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteProofBox {
        try OfflineNoteProofBox(
            backend: readField(&reader, readString),
            bytes: readField(&reader, readBytesVec)
        )
    }

    private static func readProofBoxV2(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteProofBoxV2 {
        try OfflineNoteProofBoxV2(
            backend: readField(&reader, readString),
            bytes: readField(&reader, readBytesVec)
        )
    }

    private static func readChainId(_ reader: inout OfflineNoritoReader) throws -> String {
        try readField(&reader, readString)
    }

    private static func readCommitmentOrigin(_ reader: inout OfflineNoritoReader) throws -> OfflineNoteCommitmentOrigin {
        switch try reader.readUInt32LE() {
        case 0:
            return try readField(&reader) { payload in
                try .issuerLoad(OfflineNoteIssuerLoadOrigin(
                    operationId: readField(&payload, readString),
                    lineageId: readField(&payload, readString),
                    localRevision: readField(&payload) { try $0.readUInt64LE() }
                ))
            }
        case 1:
            return try readField(&reader) { payload in
                try .p2pOutput(OfflineNoteP2pOutputOrigin(
                    paymentRequestId: readField(&payload, readString),
                    outputIndex: readField(&payload) { try $0.readUInt32LE() }
                ))
            }
        default:
            throw OfflineNoritoDecodingError.invalidField("unsupported commitment origin")
        }
    }

    private static func readAccountId(_ reader: inout OfflineNoritoReader) throws -> String {
        switch try reader.readUInt32LE() {
        case 0:
            return try readField(&reader) { payload in
                let publicKey = try readPublicKeyPayload(&payload)
                guard let algorithm = SigningAlgorithm(noritoDiscriminant: publicKey.algorithmTag) else {
                    throw OfflineNoritoDecodingError.invalidField("unsupported public key algorithm")
                }
                let address = try AccountAddress.fromAccount(
                    publicKey: publicKey.publicKey,
                    algorithm: algorithm.wireName
                )
                return try address.toI105(networkPrefix: 0x02F1)
            }
        case 1:
            return try readField(&reader) { payload in
                let canonical = try readMultisigCanonicalAccountBytes(&payload)
                return try AccountAddress.fromCanonicalBytes(canonical).toI105(networkPrefix: 0x02F1)
            }
        default:
            throw OfflineNoritoDecodingError.invalidField("unsupported account controller")
        }
    }

    private static func readPublicKeyPayload(
        _ reader: inout OfflineNoritoReader
    ) throws -> (curveId: UInt8, algorithmTag: UInt8, publicKey: Data) {
        let payload = try readConstVec(&reader)
        guard let algorithmTag = payload.first else {
            throw OfflineNoritoDecodingError.invalidField("empty public key payload")
        }
        return (try curveId(forNoritoAlgorithmTag: algorithmTag), algorithmTag, Data(payload.dropFirst()))
    }

    private static func readMultisigCanonicalAccountBytes(_ reader: inout OfflineNoritoReader) throws -> Data {
        let version = try readField(&reader) { try $0.readUInt8() }
        let threshold = try readField(&reader) { try $0.readUInt16LE() }
        let members = try readField(&reader) { payload in
            try readVec(&payload) { memberReader -> (UInt8, UInt16, Data) in
                let publicKey = try readField(&memberReader, readPublicKeyPayload)
                let weight = try readField(&memberReader) { try $0.readUInt16LE() }
                return (publicKey.curveId, weight, publicKey.publicKey)
            }
        }
        guard members.count <= Int(UInt16.max) else {
            throw OfflineNoritoDecodingError.invalidField("multisig member count overflow")
        }
        var canonical = Data([0x0A, 0x01, version])
        canonical.append(UInt8((threshold >> 8) & 0xff))
        canonical.append(UInt8(threshold & 0xff))
        let memberCount = UInt16(members.count)
        canonical.append(UInt8((memberCount >> 8) & 0xff))
        canonical.append(UInt8(memberCount & 0xff))
        for (curveId, weight, publicKey) in members {
            guard publicKey.count <= Int(UInt16.max) else {
                throw OfflineNoritoDecodingError.invalidField("multisig public key length overflow")
            }
            canonical.append(curveId)
            canonical.append(UInt8((weight >> 8) & 0xff))
            canonical.append(UInt8(weight & 0xff))
            let keyLength = UInt16(publicKey.count)
            canonical.append(UInt8((keyLength >> 8) & 0xff))
            canonical.append(UInt8(keyLength & 0xff))
            canonical.append(publicKey)
        }
        return canonical
    }

    private static func readAssetId(_ reader: inout OfflineNoritoReader) throws -> String {
        let accountId = try readField(&reader, readAccountId)
        let definitionBytes = try readField(&reader, readAssetDefinitionAddress)
        guard let definitionId = AssetDefinitionAddress.encode(uuidBytes: definitionBytes) else {
            throw OfflineNoritoDecodingError.invalidField("invalid asset definition bytes")
        }
        let dataspaceId = try readField(&reader, readAssetBalanceScope)
        let base = "\(definitionId)#\(accountId)"
        guard let dataspaceId else { return base }
        return "\(base)#dataspace:\(dataspaceId)"
    }

    private static func readAssetDefinitionAddress(_ reader: inout OfflineNoritoReader) throws -> Data {
        var out = Data()
        while reader.remaining() > 0 {
            let length = try reader.readVarint()
            guard length == 1 else {
                throw OfflineNoritoDecodingError.invalidField("asset definition byte length must be 1")
            }
            out.append(try reader.readUInt8())
        }
        return out
    }

    private static func readAssetBalanceScope(_ reader: inout OfflineNoritoReader) throws -> UInt64? {
        switch try reader.readUInt32LE() {
        case 0: return nil
        case 1: return try readField(&reader) { try $0.readUInt64LE() }
        default: throw OfflineNoritoDecodingError.invalidField("unsupported asset balance scope")
        }
    }

    private static func readNumeric(_ reader: inout OfflineNoritoReader) throws -> String {
        let mantissaBytes = try readField(&reader) { payload -> Data in
            let count = try payload.readUInt32LE()
            return try payload.readBytes(Int(count))
        }
        let scale = try readField(&reader) { try $0.readUInt32LE() }
        let (negative, digits) = decimalMagnitudeString(fromLittleEndianTwosComplement: mantissaBytes)
        return OfflineCanonicalNumeric(isNegative: negative, scale: scale, digits: digits).canonicalString
    }

    private static func decimalMagnitudeString(fromLittleEndianTwosComplement bytes: Data) -> (Bool, String) {
        guard !bytes.isEmpty else { return (false, "0") }
        var magnitude = [UInt8](bytes)
        let negative = (magnitude.last ?? 0) & 0x80 != 0
        if negative {
            for index in magnitude.indices {
                magnitude[index] = ~magnitude[index]
            }
            var carry: UInt16 = 1
            for index in magnitude.indices {
                let sum = UInt16(magnitude[index]) + carry
                magnitude[index] = UInt8(sum & 0xff)
                carry = sum >> 8
                if carry == 0 { break }
            }
        }
        while magnitude.count > 1 && magnitude.last == 0 {
            magnitude.removeLast()
        }
        var digits = "0"
        for byte in magnitude.reversed() {
            digits = multiplyDecimalString(digits, by: 256)
            digits = addDecimalString(digits, UInt16(byte))
        }
        return (negative && digits != "0", digits)
    }

    private static func multiplyDecimalString(_ value: String, by multiplier: UInt16) -> String {
        var carry: UInt16 = 0
        var out: [UInt8] = []
        for scalar in value.utf8.reversed() {
            let product = UInt16(scalar - 48) * multiplier + carry
            out.append(UInt8(product % 10) + 48)
            carry = product / 10
        }
        while carry > 0 {
            out.append(UInt8(carry % 10) + 48)
            carry /= 10
        }
        return String(bytes: out.reversed(), encoding: .utf8) ?? "0"
    }

    private static func addDecimalString(_ value: String, _ addend: UInt16) -> String {
        var carry = addend
        var out: [UInt8] = []
        for scalar in value.utf8.reversed() {
            let sum = UInt16(scalar - 48) + carry
            out.append(UInt8(sum % 10) + 48)
            carry = sum / 10
        }
        while carry > 0 {
            out.append(UInt8(carry % 10) + 48)
            carry /= 10
        }
        return String(bytes: out.reversed(), encoding: .utf8) ?? "0"
    }

    private static func curveId(forNoritoAlgorithmTag tag: UInt8) throws -> UInt8 {
        switch tag {
        case 0: return 0x01
        case 1: return 0x04
        case 2: return 0x03
        case 3: return 0x05
        case 4: return 0x02
        case 5: return 0x0A
        case 6: return 0x0B
        case 7: return 0x0C
        case 8: return 0x0D
        case 9: return 0x0E
        case 10: return 0x0F
        default: throw OfflineNoritoDecodingError.invalidField("unsupported public key compact tag")
        }
    }

    private static func requireFullyRead(_ reader: OfflineNoritoReader) throws {
        guard reader.remaining() == 0 else {
            throw OfflineNoritoDecodingError.invalidField("trailing bytes")
        }
    }
}

extension OfflineNorito {
    static func parsePublicAssetIdLiteral(_ literal: String) -> ParsedPublicAssetLiteral? {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              trimmed == literal,
              !trimmed.contains(where: \.isWhitespace) else {
            return nil
        }
        let components = trimmed.split(separator: "#", omittingEmptySubsequences: false)
        guard (components.count == 2 || components.count == 3),
              !components[0].isEmpty,
              !components[1].isEmpty else {
            return nil
        }
        let assetDefinitionId = String(components[0])
        guard AssetDefinitionAddress.decode(assetDefinitionId) != nil else {
            return nil
        }
        let accountId = String(components[1])
        guard (try? AccountAddress.parseEncoded(accountId)) != nil else {
            return nil
        }
        var dataspaceId: UInt64?
        if components.count == 3 {
            let scope = String(components[2])
            guard let rawDataspace = scope.split(
                separator: ":",
                maxSplits: 1,
                omittingEmptySubsequences: false
            ).dropFirst().first,
            scope.lowercased().hasPrefix("dataspace:"),
            !rawDataspace.isEmpty,
            let parsedDataspaceId = UInt64(rawDataspace) else {
                return nil
            }
            dataspaceId = parsedDataspaceId
        }
        return ParsedPublicAssetLiteral(
            assetDefinitionId: assetDefinitionId,
            accountId: accountId,
            dataspaceId: dataspaceId
        )
    }

    static func decodeString(_ data: Data) throws -> String {
        var reader = OfflineNoritoReader(data: data)
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8 in string")
        }
        return value
    }

    /// Decode an AccountId string from a Norito-encoded string field.
    public static func decodeAccountId(_ data: Data) throws -> String {
        return try decodeString(data)
    }

    public static func assetDefinitionIdFromLiteral(_ literal: String) -> String? {
        if let parsed = parsePublicAssetIdLiteral(literal) {
            return parsed.assetDefinitionId
        }
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }
        guard !trimmed.contains("#"),
              AssetDefinitionAddress.decode(trimmed) != nil else {
            return nil
        }
        return trimmed
    }

    static func accountIdFromLiteral(_ literal: String) -> String? {
        parsePublicAssetIdLiteral(literal)?.accountId
    }
}
