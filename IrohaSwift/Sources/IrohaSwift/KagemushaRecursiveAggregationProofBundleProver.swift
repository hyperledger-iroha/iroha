import Foundation

public enum KagemushaRecursiveAggregationProofBundleProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case emptyPallasOpenEnvelopesArchive
    case bridgeUnavailable
    case proofRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not be empty."
        case .emptyPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must not be empty."
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
    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveAggregationProofBundleProverAvailable
    }

    public static func proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaRecursiveAggregationProofBundleProverError.emptyRecordBundleArchive
        }
        guard !pallasOpenEnvelopesArchive.isEmpty else {
            throw KagemushaRecursiveAggregationProofBundleProverError.emptyPallasOpenEnvelopesArchive
        }
        guard NoritoNativeBridge.shared.isKagemushaRecursiveAggregationProofBundleProverAvailable else {
            throw KagemushaRecursiveAggregationProofBundleProverError.bridgeUnavailable
        }
        do {
            guard let proofBundle = try NoritoNativeBridge.shared
                .proveKagemushaVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: recordBundleArchive,
                    pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive
                )
            else {
                throw KagemushaRecursiveAggregationProofBundleProverError.bridgeUnavailable
            }
            return proofBundle
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveAggregationProofBundleProverError.proofRejected
        } catch {
            throw KagemushaRecursiveAggregationProofBundleProverError.proofRejected
        }
    }
}
