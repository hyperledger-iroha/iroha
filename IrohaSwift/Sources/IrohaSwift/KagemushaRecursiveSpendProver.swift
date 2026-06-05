import Foundation

public enum KagemushaRecursiveSpendProverError: Error, Equatable, LocalizedError {
    case emptyRequestArchive
    case bridgeUnavailable
    case proofRejected
    case oversizedNativeOutput

    public var errorDescription: String? {
        switch self {
        case .emptyRequestArchive:
            return "Kagemusha recursive spend request archive must not be empty."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive spend native bridge is unavailable."
            )
        case .proofRejected:
            return "Kagemusha recursive spend request was rejected by the native bridge."
        case .oversizedNativeOutput:
            return "Kagemusha recursive spend native bridge returned an oversized archive."
        }
    }
}

public enum KagemushaOfflineSpendMode: String, Equatable {
    case recursiveSpendV1 = "recursive_spend_v1"
    case checkedPrefoldV1 = "checked_prefold_v1"
}

public enum KagemushaRecursiveSpendProver {
    public static let requiredBridgeAbiVersion: UInt32 = 6
    public static let recursiveAggregationProofCircuitIdV1 = "kagemusha-recursive-aggregation-v1"
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

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
    }

    public static var preferredMode: KagemushaOfflineSpendMode {
        preferredMode(recursiveSpendAvailable: isNativeAvailable)
    }

    public static func preferredMode(recursiveSpendAvailable: Bool) -> KagemushaOfflineSpendMode {
        recursiveSpendAvailable ? .recursiveSpendV1 : .checkedPrefoldV1
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
        return archive
    }
}
