import Foundation

public enum KagemushaRecursiveAggregationProofBundleProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case emptyPallasOpenEnvelopesArchive
    case invalidRecordBundleArchive
    case emptyRecordBundlePayload
    case invalidPallasOpenEnvelopesArchive
    case emptyPallasOpenEnvelopesPayload
    case oversizedProofBundleArchive
    case invalidProofBundleArchive
    case emptyProofBundlePayload
    case bridgeUnavailable
    case proofRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not be empty."
        case .emptyPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must not be empty."
        case .invalidRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must be a valid Norito archive."
        case .emptyRecordBundlePayload:
            return "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload."
        case .invalidPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must be a valid Norito archive."
        case .emptyPallasOpenEnvelopesPayload:
            return "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload."
        case .oversizedProofBundleArchive:
            return "Kagemusha recursive aggregation native bridge returned an oversized archive."
        case .invalidProofBundleArchive:
            return "Kagemusha recursive aggregation native bridge returned an invalid Norito archive."
        case .emptyProofBundlePayload:
            return "Kagemusha recursive aggregation native bridge returned an empty Norito payload."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive aggregation proof-bundle prover is unavailable."
            )
        case .proofRejected:
            return "Kagemusha recursive aggregation inputs were rejected by the native prover."
        }
    }
}

public enum KagemushaRecursiveAggregationProofBundleProver {
    private static let maxNoritoHeaderPaddingBytes = 64

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveAggregationProofBundleProverAvailable
    }

    public static func proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data
    ) throws -> Data {
        try proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: recordBundleArchive,
            pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveAggregationProofBundleProverAvailable
        ) {
            try NoritoNativeBridge.shared
                .proveKagemushaVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: recordBundleArchive,
                    pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive
                )
        }
    }

    static func proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Data?
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaRecursiveAggregationProofBundleProverError.emptyRecordBundleArchive
        }
        guard !pallasOpenEnvelopesArchive.isEmpty else {
            throw KagemushaRecursiveAggregationProofBundleProverError.emptyPallasOpenEnvelopesArchive
        }
        try requireValidInputArchive(
            recordBundleArchive,
            invalidError: .invalidRecordBundleArchive,
            emptyPayloadError: .emptyRecordBundlePayload
        )
        try requireValidInputArchive(
            pallasOpenEnvelopesArchive,
            invalidError: .invalidPallasOpenEnvelopesArchive,
            emptyPayloadError: .emptyPallasOpenEnvelopesPayload
        )
        guard bridgeAvailable else {
            throw KagemushaRecursiveAggregationProofBundleProverError.bridgeUnavailable
        }
        let proofBundle: Data?
        do {
            proofBundle = try body()
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveAggregationProofBundleProverError.proofRejected
        } catch {
            throw KagemushaRecursiveAggregationProofBundleProverError.proofRejected
        }
        guard let proofBundle else {
            throw KagemushaRecursiveAggregationProofBundleProverError.bridgeUnavailable
        }
        guard !proofBundle.isEmpty else {
            throw KagemushaRecursiveAggregationProofBundleProverError.proofRejected
        }
        try requireValidProofBundleArchive(proofBundle)
        return proofBundle
    }

    private static func requireValidInputArchive(
        _ archive: Data,
        invalidError: KagemushaRecursiveAggregationProofBundleProverError,
        emptyPayloadError: KagemushaRecursiveAggregationProofBundleProverError
    ) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw invalidError
        }
        guard frame.header.length > 0 else {
            throw emptyPayloadError
        }
    }

    private static func requireValidProofBundleArchive(_ archive: Data) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes else {
            throw KagemushaRecursiveAggregationProofBundleProverError.oversizedProofBundleArchive
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw KagemushaRecursiveAggregationProofBundleProverError.invalidProofBundleArchive
        }
        guard frame.header.length > 0 else {
            throw KagemushaRecursiveAggregationProofBundleProverError.emptyProofBundlePayload
        }
    }
}
