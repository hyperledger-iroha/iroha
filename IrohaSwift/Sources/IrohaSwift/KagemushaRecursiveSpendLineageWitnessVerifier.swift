import Foundation

public enum KagemushaRecursiveSpendLineageWitnessVerifierError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case nativeVerifierUnavailable

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha lineage-witness verification field: \(field)."
        case .nativeVerifierUnavailable:
            return "The native Kagemusha lineage-witness verifier entrypoint is not available in this bridge."
        }
    }
}

/// Typed input for standalone lineage-witness verification.
///
/// Redeem currently verifies the same material internally. This separate DTO
/// prevents wallet code from treating structural archive parsing as
/// cryptographic verification while the dedicated native entrypoint is being
/// added.
public struct KagemushaRecursiveSpendLineageWitnessVerifyRequest: Equatable, Sendable {
    public let bundle: Data
    public let lineageWitness: Data
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let lineageVerifierRecords: [KagemushaRecursiveSpendVerifierRecordRef]
    public let blockHeight: UInt64?

    public init(
        bundle: Data,
        lineageWitness: Data,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        lineageVerifierRecords: [KagemushaRecursiveSpendVerifierRecordRef] = [],
        blockHeight: UInt64? = nil
    ) throws {
        let summary = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(bundle)
        _ = summary
        try KagemushaRecursiveSpendRequestCodecs.requireNestedArchive(
            lineageWitness,
            field: "lineageWitness"
        )
        let ids = [lineageVerifierRecord].compactMap { $0?.verifierKeyId }
            + lineageVerifierRecords.map(\.verifierKeyId)
        guard Set(ids).count == ids.count else {
            throw KagemushaRecursiveSpendLineageWitnessVerifierError.invalidField(
                "lineageVerifierRecords"
            )
        }
        self.bundle = Data(bundle)
        self.lineageWitness = Data(lineageWitness)
        self.lineageVerifierRecord = lineageVerifierRecord
        self.lineageVerifierRecords = lineageVerifierRecords
        self.blockHeight = blockHeight
    }
}

public enum KagemushaRecursiveSpendLineageWitnessVerifier {
    /// ABI 17 verifies witnesses as part of redemption but does not export a
    /// standalone verifier symbol yet.
    public static let isNativeAvailable = false
    public static let requestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessVerifyRequestV1"
    public static let requiredNativeSymbol =
        "connect_norito_kagemusha_recursive_spend_lineage_witness_verify"

    public static func verify(
        _ request: KagemushaRecursiveSpendLineageWitnessVerifyRequest
    ) throws -> Never {
        _ = request
        throw KagemushaRecursiveSpendLineageWitnessVerifierError.nativeVerifierUnavailable
    }
}
