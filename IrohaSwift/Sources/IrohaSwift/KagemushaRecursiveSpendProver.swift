import Foundation
import CryptoKit

public enum KagemushaRecursiveSpendProverError: Error, Equatable, LocalizedError {
    case emptyRequestArchive
    case oversizedInputArchive
    case invalidInputArchive
    case emptyInputPayload
    case bridgeUnavailable
    case proofRejected
    case oversizedNativeOutput
    case invalidNativeOutput
    case emptyNativeOutputPayload
    case invalidLineageKeyArtifact(String)

    public var errorDescription: String? {
        switch self {
        case .emptyRequestArchive:
            return "Kagemusha recursive spend request archive must not be empty."
        case .oversizedInputArchive:
            return "Kagemusha recursive spend input archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .invalidInputArchive:
            return "Kagemusha recursive spend input archive must be a valid Norito archive."
        case .emptyInputPayload:
            return "Kagemusha recursive spend input archive must contain a non-empty Norito payload."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive spend native bridge is unavailable."
            )
        case .proofRejected:
            return "Kagemusha recursive spend request was rejected by the native bridge."
        case .oversizedNativeOutput:
            return "Kagemusha recursive spend native bridge returned an oversized archive."
        case .invalidNativeOutput:
            return "Kagemusha recursive spend native bridge returned an invalid Norito archive."
        case .emptyNativeOutputPayload:
            return "Kagemusha recursive spend native bridge returned an empty Norito payload."
        case let .invalidLineageKeyArtifact(field):
            return "Kagemusha recursive spend lineage key artifact is invalid: \(field)."
        }
    }
}

public enum KagemushaOfflineSpendMode: String, Equatable {
    case recursiveCompactV1 = "recursive_compact_v1"
    case recursiveSpendV1 = "recursive_spend_v1"
    case checkedPrefoldV1 = "checked_prefold_v1"
}

public enum KagemushaRecursiveSpendProver {
    public static let requiredBridgeAbiVersion: UInt32 = 6
    public static let recursiveCompactRequiredBridgeAbiVersion: UInt32 = 7
    public static let recursiveAggregationProofCircuitIdV1 = "kagemusha-recursive-aggregation-v1"
    public static let recursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1"
    public static let recursiveAggregationProofBackend = "halo2/ipa"
    public static let recursiveSpendLineageProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-v1"
    public static let recursiveSpendLineageOneHopProofCircuitIdV1 =
        "kagemusha-recursive-spend-lineage-onehop-v1"
    public static let recursiveSpendLineageAppendProofCircuitIdV1 =
        "kagemusha-recursive-spend-lineage-append-v1"
    public static let compactTokenMaxHops: UInt32 = 64
    public static let recursiveSpendLineageWitnesslessMaxHopsV1: UInt32 = 64
    public static let recursiveSpendLineageTransitionCircuitWiredV1 = true
    public static let recursivePreviousProofOpenEnvelopesRequiredCountV1 = 1
    public static let recursivePreviousProofOpenEnvelopesMaxBytes = 8 * 1024 * 1024
    public static let recursivePallasOpenEnvelopeMaxTranscriptLabelBytes = 128
    public static let nativeArchiveMaxBytes = 64 * 1024 * 1024
    public static let recursiveSpendTransitionProfileDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile"
    public static let recursiveSpendTransitionProfileDigestDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
    public static let recursiveSpendTransitionProfileBindingDigestDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
    public static let recursiveSpendLineageAppendOpeningsPreflightDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
    public static let recursiveSpendLineageAppendBoundaryDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
    public static let recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
    public static let recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
    private static let maxNoritoHeaderPaddingBytes = 64
    private static let kagemushaNoritoCompactLenFlag = NoritoHeader.compactLen
    private static let kagemushaNoritoPackedStructFlag = NoritoHeader.packedStruct
    private static let privacyNoritoFieldBitsetFlag = NoritoHeader.fieldBitset
    private static let kagemushaLineageProvingKeyArchiveVersionV1: UInt16 = 1
    private static let kagemushaLineageProvingKeyArchiveSchemaHash: [UInt8] = [
        0xc8, 0x84, 0x89, 0x61, 0x8a, 0x01, 0x2c, 0x28,
        0x3f, 0xf3, 0xbb, 0x2e, 0xba, 0xbc, 0x77, 0x75,
    ]
    private static let kagemushaZk1Magic = Data([0x5A, 0x4B, 0x31, 0x00])
    private static let kagemushaZk1TlvCid1 = Data("CID1".utf8)
    private static let kagemushaZk1TlvIpaK = Data("IPAK".utf8)
    private static let kagemushaZk1TlvH2Vk = Data("H2VK".utf8)

    private struct LineageProvingKeyArchive {
        let version: UInt16
        let circuitFamily: String
        let verifierKeyCommitment: Data
        let provingKey: Data
    }

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
    }

    public static var preferredMode: KagemushaOfflineSpendMode {
        preferredMode(
            recursiveCompactAvailable: KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable,
            recursiveSpendAvailable: isNativeAvailable
        )
    }

    public static func preferredMode(recursiveSpendAvailable: Bool) -> KagemushaOfflineSpendMode {
        preferredMode(
            recursiveCompactAvailable: false,
            recursiveSpendAvailable: recursiveSpendAvailable
        )
    }

    public static func preferredMode(
        recursiveCompactAvailable: Bool,
        recursiveSpendAvailable: Bool
    ) -> KagemushaOfflineSpendMode {
        _ = recursiveCompactAvailable
        return recursiveSpendAvailable ? .recursiveSpendV1 : .checkedPrefoldV1
    }

    public static func canRedeemWitnessless(circuitId: String, hopCount: UInt32) -> Bool {
        recursiveSpendLineageTransitionCircuitWiredV1
            && isLineageProofCircuitId(circuitId)
            && hopCount >= 1
            && hopCount <= recursiveSpendLineageWitnesslessMaxHopsV1
    }

    public static func isLineageProofCircuitId(_ circuitId: String?) -> Bool {
        circuitId == recursiveSpendLineageProofCircuitIdV1
            || circuitId == recursiveSpendLineageOneHopProofCircuitIdV1
            || circuitId == recursiveSpendLineageAppendProofCircuitIdV1
    }

    public static func isLineageAppendOutputCircuitId(_ outputCircuitId: String?) -> Bool {
        outputCircuitId == recursiveSpendLineageProofCircuitIdV1
            || outputCircuitId == recursiveSpendLineageAppendProofCircuitIdV1
    }

    public static func isSupportedLineageKeyArtifactOpeningLen(_ verifierOpeningLen: UInt32) -> Bool {
        switch verifierOpeningLen {
        case 2, 4, 8, 16, 32, 64, 128:
            return true
        default:
            return false
        }
    }

    public struct LineageKeyArtifacts: Equatable {
        public let proofCircuitId: String
        public let verifierOpeningLen: UInt32
        public let lineageVerifierKeyBackend: String
        public let lineageVerifierKey: Data
        public let lineageProvingKeyArchive: Data

        public var isInitArtifact: Bool {
            proofCircuitId == KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
        }

        public var isAppendArtifact: Bool {
            proofCircuitId == KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        }

        public init(
            proofCircuitId: String,
            verifierOpeningLen: UInt32,
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: Data,
            lineageProvingKeyArchive: Data
        ) throws {
            try KagemushaRecursiveSpendProver.validateLineageKeyArtifactFields(
                proofCircuitId: proofCircuitId,
                verifierOpeningLen: verifierOpeningLen,
                lineageVerifierKeyBackend: lineageVerifierKeyBackend,
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
            self.proofCircuitId = proofCircuitId
            self.verifierOpeningLen = verifierOpeningLen
            self.lineageVerifierKeyBackend = lineageVerifierKeyBackend
            self.lineageVerifierKey = lineageVerifierKey
            self.lineageProvingKeyArchive = lineageProvingKeyArchive
        }
    }

    public static func lineageKeyArtifactsForInit(
        verifierOpeningLen: UInt32,
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws -> LineageKeyArtifacts {
        try lineageKeyArtifacts(
            proofCircuitId: recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierOpeningLen: verifierOpeningLen,
            lineageVerifierKeyBackend: lineageVerifierKeyBackend,
            lineageVerifierKey: lineageVerifierKey,
            lineageProvingKeyArchive: lineageProvingKeyArchive
        )
    }

    public static func lineageKeyArtifactsForAppend(
        verifierOpeningLen: UInt32,
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws -> LineageKeyArtifacts {
        try lineageKeyArtifacts(
            proofCircuitId: recursiveSpendLineageAppendProofCircuitIdV1,
            verifierOpeningLen: verifierOpeningLen,
            lineageVerifierKeyBackend: lineageVerifierKeyBackend,
            lineageVerifierKey: lineageVerifierKey,
            lineageProvingKeyArchive: lineageProvingKeyArchive
        )
    }

    public static func lineageKeyArtifacts(
        proofCircuitId: String,
        verifierOpeningLen: UInt32,
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws -> LineageKeyArtifacts {
        try LineageKeyArtifacts(
            proofCircuitId: proofCircuitId,
            verifierOpeningLen: verifierOpeningLen,
            lineageVerifierKeyBackend: lineageVerifierKeyBackend,
            lineageVerifierKey: lineageVerifierKey,
            lineageProvingKeyArchive: lineageProvingKeyArchive
        )
    }

    public static func validateLineageKeyArtifacts(_ artifacts: LineageKeyArtifacts) throws -> LineageKeyArtifacts {
        try validateLineageKeyArtifactFields(
            proofCircuitId: artifacts.proofCircuitId,
            verifierOpeningLen: artifacts.verifierOpeningLen,
            lineageVerifierKeyBackend: artifacts.lineageVerifierKeyBackend,
            lineageVerifierKey: artifacts.lineageVerifierKey,
            lineageProvingKeyArchive: artifacts.lineageProvingKeyArchive
        )
        return artifacts
    }

    private static func validateLineageKeyArtifactFields(
        proofCircuitId: String,
        verifierOpeningLen: UInt32,
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws {
        guard proofCircuitId == recursiveSpendLineageOneHopProofCircuitIdV1
            || proofCircuitId == recursiveSpendLineageAppendProofCircuitIdV1
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("proof_circuit_id")
        }
        guard isSupportedLineageKeyArtifactOpeningLen(verifierOpeningLen) else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("verifier_opening_len")
        }
        guard lineageVerifierKeyBackend == recursiveAggregationProofBackend,
              !lineageVerifierKey.isEmpty
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
        }
        guard !lineageProvingKeyArchive.isEmpty else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        try validateLineageKeyArtifactPackageBinding(
            proofCircuitId: proofCircuitId,
            lineageVerifierKeyBackend: lineageVerifierKeyBackend,
            lineageVerifierKey: lineageVerifierKey,
            lineageProvingKeyArchive: lineageProvingKeyArchive
        )
    }

    private static func validateLineageKeyArtifactPackageBinding(
        proofCircuitId: String,
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws {
        let verifierCircuitId = try lineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey)
        guard verifierCircuitId == proofCircuitId else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
        }
        let archivePayload = try lineageProvingKeyArchivePayload(lineageProvingKeyArchive)
        let circuitIdBytes = Data(proofCircuitId.utf8)
        let verifierKeyCommitment = verifyingKeyCommitment(
            lineageVerifierKeyBackend: lineageVerifierKeyBackend,
            lineageVerifierKey: lineageVerifierKey
        )
        guard archivePayload.range(of: circuitIdBytes) != nil,
              archivePayload.range(of: verifierKeyCommitment) != nil
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let archive = try decodeLineageProvingKeyArchivePayload(
            archivePayload,
            flags: lineageProvingKeyArchive[NoritoHeader.encodedLength - 1]
        )
        guard archive.version == kagemushaLineageProvingKeyArchiveVersionV1,
              archive.circuitFamily == proofCircuitId,
              archive.verifierKeyCommitment == verifierKeyCommitment,
              !archive.provingKey.isEmpty
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
    }

    private static func lineageVerifierKeyEnvelopeCircuitId(_ lineageVerifierKey: Data) throws -> String {
        guard lineageVerifierKey.starts(with: kagemushaZk1Magic) else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
        }
        var offset = kagemushaZk1Magic.count
        var circuitId: String?
        var sawIpaK = false
        var sawH2Vk = false
        while offset < lineageVerifierKey.count {
            guard offset + 8 <= lineageVerifierKey.count,
                  let payloadLength = readUInt32LE(lineageVerifierKey, at: offset + 4)
            else {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
            }
            let payloadStart = offset + 8
            let payloadEnd = payloadStart + Int(payloadLength)
            guard payloadEnd <= lineageVerifierKey.count else {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
            }
            let tag = Data(lineageVerifierKey[offset..<(offset + 4)])
            let payload = Data(lineageVerifierKey[payloadStart..<payloadEnd])
            if tag == kagemushaZk1TlvCid1 {
                guard circuitId == nil,
                      !payload.isEmpty,
                      !payload.contains(where: { $0 < 0x20 || $0 > 0x7E }),
                      let decoded = String(data: payload, encoding: .utf8)
                else {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
                }
                let trimmed = decoded.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
                }
                circuitId = trimmed
            } else if tag == kagemushaZk1TlvIpaK {
                guard !sawIpaK, payload.count == 4 else {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
                }
                sawIpaK = true
            } else if tag == kagemushaZk1TlvH2Vk {
                guard !sawH2Vk, !payload.isEmpty else {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
                }
                sawH2Vk = true
            } else {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
            }
            offset = payloadEnd
        }
        guard let circuitId, sawIpaK, sawH2Vk else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_verifier_key")
        }
        return circuitId
    }

    private static func lineageProvingKeyArchivePayload(_ lineageProvingKeyArchive: Data) throws -> Data {
        guard let frame = noritoDecodeFrame(lineageProvingKeyArchive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes,
              frame.header.schema == kagemushaLineageProvingKeyArchiveSchemaHash,
              (frame.header.flags & kagemushaNoritoPackedStructFlag) == 0,
              (frame.header.flags & privacyNoritoFieldBitsetFlag) == 0,
              frame.header.length > 0
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        return frame.payload
    }

    private static func decodeLineageProvingKeyArchivePayload(
        _ payload: Data,
        flags: UInt8
    ) throws -> LineageProvingKeyArchive {
        var offset = 0
        let versionPayload = try readNoritoField(payload, offset: &offset, flags: flags)
        guard versionPayload.count == 2,
              let version = readUInt16LE(versionPayload, at: 0)
        else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let circuitFamilyPayload = try readNoritoField(payload, offset: &offset, flags: flags)
        let circuitFamily = try decodeNoritoString(circuitFamilyPayload, flags: flags)
        let verifierKeyCommitment = try readNoritoField(payload, offset: &offset, flags: flags)
        guard verifierKeyCommitment.count == 32 else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let provingKeyPayload = try readNoritoField(payload, offset: &offset, flags: flags)
        let provingKey = try decodeNoritoByteVec(provingKeyPayload)
        guard offset == payload.count else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        return LineageProvingKeyArchive(
            version: version,
            circuitFamily: circuitFamily,
            verifierKeyCommitment: verifierKeyCommitment,
            provingKey: provingKey
        )
    }

    private static func readNoritoField(
        _ buffer: Data,
        offset: inout Int,
        flags: UInt8
    ) throws -> Data {
        let (length, payloadStart) = try readNoritoLength(buffer, offset: offset, flags: flags)
        let payloadEnd = payloadStart + length
        guard payloadEnd <= buffer.count else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let startIndex = buffer.index(buffer.startIndex, offsetBy: payloadStart)
        let endIndex = buffer.index(buffer.startIndex, offsetBy: payloadEnd)
        offset = payloadEnd
        return Data(buffer[startIndex..<endIndex])
    }

    private static func readNoritoLength(
        _ buffer: Data,
        offset: Int,
        flags: UInt8
    ) throws -> (Int, Int) {
        guard offset >= 0 else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        if (flags & kagemushaNoritoCompactLenFlag) == 0 {
            guard let value = readUInt64LE(buffer, at: offset),
                  value <= UInt64(Int.max),
                  value <= UInt64(buffer.count)
            else {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
            }
            return (Int(value), offset + 8)
        }

        var value: UInt64 = 0
        var shift: UInt64 = 0
        var cursor = offset
        for _ in 0..<10 {
            guard cursor < buffer.count else {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
            }
            let byte = buffer[buffer.index(buffer.startIndex, offsetBy: cursor)]
            cursor += 1
            let chunk = UInt64(byte & 0x7f)
            if shift >= 63 && chunk > 1 {
                throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
            }
            value |= chunk << shift
            if (byte & 0x80) == 0 {
                let encodedLength = cursor - offset
                if encodedLength > 1 && value < (UInt64(1) << UInt64(7 * (encodedLength - 1))) {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
                }
                guard value <= UInt64(Int.max),
                      value <= UInt64(buffer.count)
                else {
                    throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
                }
                return (Int(value), cursor)
            }
            shift += 7
        }
        throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
    }

    private static func decodeNoritoString(_ payload: Data, flags: UInt8) throws -> String {
        let (length, start) = try readNoritoLength(payload, offset: 0, flags: flags)
        let end = start + length
        guard end == payload.count else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let startIndex = payload.index(payload.startIndex, offsetBy: start)
        let endIndex = payload.index(payload.startIndex, offsetBy: end)
        guard let decoded = String(data: Data(payload[startIndex..<endIndex]), encoding: .utf8) else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        return decoded
    }

    private static func decodeNoritoByteVec(_ payload: Data) throws -> Data {
        guard let length = readUInt64LE(payload, at: 0) else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let available = payload.count - 8
        guard length == UInt64(available) else {
            throw KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact("lineage_proving_key_archive")
        }
        let startIndex = payload.index(payload.startIndex, offsetBy: 8)
        let endIndex = payload.index(payload.startIndex, offsetBy: payload.count)
        return Data(payload[startIndex..<endIndex])
    }

    private static func verifyingKeyCommitment(
        lineageVerifierKeyBackend: String,
        lineageVerifierKey: Data
    ) -> Data {
        let backend = Data(lineageVerifierKeyBackend.utf8)
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.count), to: &preimage)
        preimage.append(backend)
        appendUInt64BE(UInt64(lineageVerifierKey.count), to: &preimage)
        preimage.append(lineageVerifierKey)
        return Data(SHA256.hash(data: preimage))
    }

    private static func readUInt32LE(_ data: Data, at offset: Int) -> UInt32? {
        guard offset >= 0, offset + 4 <= data.count else {
            return nil
        }
        var value: UInt32 = 0
        data[offset..<(offset + 4)].withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            memcpy(&value, baseAddress, 4)
        }
        return UInt32(littleEndian: value)
    }

    private static func readUInt16LE(_ data: Data, at offset: Int) -> UInt16? {
        guard offset >= 0, offset + 2 <= data.count else {
            return nil
        }
        var value: UInt16 = 0
        data[offset..<(offset + 2)].withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            memcpy(&value, baseAddress, 2)
        }
        return UInt16(littleEndian: value)
    }

    private static func readUInt64LE(_ data: Data, at offset: Int) -> UInt64? {
        guard offset >= 0, offset + 8 <= data.count else {
            return nil
        }
        var value: UInt64 = 0
        data[offset..<(offset + 8)].withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            memcpy(&value, baseAddress, 8)
        }
        return UInt64(littleEndian: value)
    }

    private static func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        var bigEndian = value.bigEndian
        withUnsafeBytes(of: &bigEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    public static func requiresLineageKeyArtifactsForInit() -> Bool {
        true
    }

    public static func requiresLineageWitnessForRedeem(circuitId: String, hopCount: UInt32) -> Bool {
        !canRedeemWitnessless(circuitId: circuitId, hopCount: hopCount)
    }

    public static func canAppendWitnesslessLineage(previousHopCount: UInt32) -> Bool {
        recursiveSpendLineageTransitionCircuitWiredV1
            && previousHopCount >= 1
            && previousHopCount < recursiveSpendLineageWitnesslessMaxHopsV1
    }

    public static func normalizedAppendOutputCircuitId(_ outputCircuitId: String?) -> String {
        guard let outputCircuitId, !outputCircuitId.isEmpty else {
            return recursiveAggregationProofCircuitIdV1
        }
        if outputCircuitId == recursiveSpendLineageProofCircuitIdV1 {
            return recursiveSpendLineageAppendProofCircuitIdV1
        }
        return outputCircuitId
    }

    public static func isSupportedAppendOutputCircuitId(_ outputCircuitId: String?) -> Bool {
        let normalized = normalizedAppendOutputCircuitId(outputCircuitId)
        return normalized == recursiveAggregationProofCircuitIdV1
            || normalized == recursiveSpendLineageAppendProofCircuitIdV1
    }

    public static func requiresLineageKeyArtifactsForAppendOutput(outputCircuitId: String?) -> Bool {
        isLineageAppendOutputCircuitId(normalizedAppendOutputCircuitId(outputCircuitId))
    }

    public static func isSupportedPreviousProofCircuitId(_ previousProofCircuitId: String?) -> Bool {
        previousProofCircuitId == recursiveAggregationProofCircuitIdV1
            || isLineageProofCircuitId(previousProofCircuitId)
    }

    public static func requiresPreviousLineageVerifierRecordForAppend(
        previousProofCircuitId: String?
    ) -> Bool {
        isLineageProofCircuitId(previousProofCircuitId)
    }

    public static func isSupportedAppendProofTransition(
        previousProofCircuitId: String?,
        outputCircuitId: String?
    ) -> Bool {
        let normalizedOutput = normalizedAppendOutputCircuitId(outputCircuitId)
        return (previousProofCircuitId == recursiveAggregationProofCircuitIdV1
            && normalizedOutput == recursiveAggregationProofCircuitIdV1)
            || (isLineageProofCircuitId(previousProofCircuitId)
                && (
                    normalizedOutput == recursiveAggregationProofCircuitIdV1
                        || normalizedOutput == recursiveSpendLineageAppendProofCircuitIdV1
                ))
    }

    public static func preferredAppendOutputCircuitId(previousHopCount: UInt32) -> String {
        canAppendWitnesslessLineage(previousHopCount: previousHopCount)
            ? recursiveSpendLineageAppendProofCircuitIdV1
            : recursiveAggregationProofCircuitIdV1
    }

    public static func canProveAppendOutputCircuitId(
        _ outputCircuitId: String?,
        previousHopCount: UInt32
    ) -> Bool {
        guard previousHopCount >= 1 else {
            return false
        }
        switch normalizedAppendOutputCircuitId(outputCircuitId) {
        case recursiveAggregationProofCircuitIdV1:
            return previousHopCount < compactTokenMaxHops
        case recursiveSpendLineageAppendProofCircuitIdV1:
            return canAppendWitnesslessLineage(previousHopCount: previousHopCount)
        default:
            return false
        }
    }

    public static func canSelectAppendOutputCircuitId(
        previousProofCircuitId: String?,
        outputCircuitId: String?,
        previousHopCount: UInt32
    ) -> Bool {
        guard canProveAppendOutputCircuitId(outputCircuitId, previousHopCount: previousHopCount) else {
            return false
        }
        guard isSupportedPreviousProofCircuitId(previousProofCircuitId) else {
            return false
        }
        return isSupportedAppendProofTransition(
            previousProofCircuitId: previousProofCircuitId,
            outputCircuitId: outputCircuitId
        )
    }

    public static func requiresPreviousProofOpenEnvelopesForAppend(
        outputCircuitId: String?,
        previousHopCount: UInt32
    ) -> Bool {
        isLineageAppendOutputCircuitId(normalizedAppendOutputCircuitId(outputCircuitId))
            && previousHopCount >= 1
    }

    public static func initSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendInit(requestArchive: $0)
        }
    }

    public static func appendSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppend(requestArchive: $0)
        }
    }

    public static func transitionProfileInit(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendTransitionProfileInit(
                requestArchive: $0
            )
        }
    }

    public static func transitionProfileAppend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendTransitionProfileAppend(
                requestArchive: $0
            )
        }
    }

    public static func lineageAppendBoundary(profileArchive: Data) throws -> Data {
        try call(
            requestArchive: profileArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendLineageAppendBoundary(
                profileArchive: $0
            )
        }
    }

    public static func lineageWitnessFromInitResult(
        requestArchive: Data,
        bundleArchive: Data
    ) throws -> Data {
        try call(
            archives: [requestArchive, bundleArchive],
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendLineageWitnessFromInitResult(
                requestArchive: requestArchive,
                bundleArchive: bundleArchive
            )
        }
    }

    public static func lineageWitnessAppendResult(
        previousWitnessArchive: Data,
        requestArchive: Data,
        bundleArchive: Data
    ) throws -> Data {
        try call(
            archives: [previousWitnessArchive, requestArchive, bundleArchive],
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendLineageWitnessAppendResult(
                previousWitnessArchive: previousWitnessArchive,
                requestArchive: requestArchive,
                bundleArchive: bundleArchive
            )
        }
    }

    public static func verifySpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendVerify(requestArchive: $0)
        }
    }

    public static func redeemSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendRedeem(requestArchive: $0)
        }
    }

    static func call(
        requestArchive: Data,
        bridgeAvailable: Bool,
        _ body: (Data) throws -> Data?
    ) throws -> Data {
        try call(
            archives: [requestArchive],
            bridgeAvailable: bridgeAvailable
        ) {
            try body(requestArchive)
        }
    }

    static func call(
        archives: [Data],
        bridgeAvailable: Bool,
        _ body: () throws -> Data?
    ) throws -> Data {
        guard archives.allSatisfy({ !$0.isEmpty }) else {
            throw KagemushaRecursiveSpendProverError.emptyRequestArchive
        }
        try archives.forEach(requireValidInputArchive)
        guard bridgeAvailable else {
            throw KagemushaRecursiveSpendProverError.bridgeUnavailable
        }
        let archive: Data?
        do {
            archive = try body()
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveSpendProverError.proofRejected
        } catch {
            throw KagemushaRecursiveSpendProverError.proofRejected
        }
        guard let archive else {
            throw KagemushaRecursiveSpendProverError.bridgeUnavailable
        }
        guard !archive.isEmpty else {
            throw KagemushaRecursiveSpendProverError.proofRejected
        }
        guard archive.count <= nativeArchiveMaxBytes else {
            throw KagemushaRecursiveSpendProverError.oversizedNativeOutput
        }
        try requireValidOutputArchive(archive)
        return archive
    }

    private static func requireValidInputArchive(_ archive: Data) throws {
        try requireValidNoritoArchive(
            archive,
            oversizedError: .oversizedInputArchive,
            invalidError: .invalidInputArchive,
            emptyPayloadError: .emptyInputPayload
        )
    }

    private static func requireValidOutputArchive(_ archive: Data) throws {
        try requireValidNoritoArchive(
            archive,
            oversizedError: .oversizedNativeOutput,
            invalidError: .invalidNativeOutput,
            emptyPayloadError: .emptyNativeOutputPayload
        )
    }

    private static func requireValidNoritoArchive(
        _ archive: Data,
        oversizedError: KagemushaRecursiveSpendProverError,
        invalidError: KagemushaRecursiveSpendProverError,
        emptyPayloadError: KagemushaRecursiveSpendProverError
    ) throws {
        guard archive.count <= nativeArchiveMaxBytes else {
            throw oversizedError
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes
        else {
            throw invalidError
        }
        guard frame.header.length > 0 else {
            throw emptyPayloadError
        }
    }
}
