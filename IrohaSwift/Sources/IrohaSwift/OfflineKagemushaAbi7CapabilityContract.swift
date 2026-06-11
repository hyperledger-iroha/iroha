import Foundation

public enum OfflineKagemushaAbi7CapabilityError: Error, Equatable, LocalizedError, Sendable {
    case offlinePaymentsDisabled
    case lifecycleDisabled
    case unsupportedMode(expected: String, actual: String?)
    case unsupportedNativeBridgeAbi(expected: UInt32, actual: UInt32?)
    case unsupportedCircuitId(expected: String, actual: String?)
    case missingArtifactSetId
    case artifactsUnavailable

    public var errorDescription: String? {
        switch self {
        case .offlinePaymentsDisabled:
            return "Offline payments are disabled."
        case .lifecycleDisabled:
            return "Kagemusha ABI-7 offline lifecycle is disabled."
        case let .unsupportedMode(expected, actual):
            return "Kagemusha ABI-7 mode must be \(expected), got \(actual ?? "missing")."
        case let .unsupportedNativeBridgeAbi(expected, actual):
            return "Kagemusha ABI-7 native bridge ABI must be \(expected), got \(actual.map(String.init) ?? "missing")."
        case let .unsupportedCircuitId(expected, actual):
            return "Kagemusha ABI-7 circuit id must be \(expected), got \(actual ?? "missing")."
        case .missingArtifactSetId:
            return "Kagemusha ABI-7 artifact set id is missing."
        case .artifactsUnavailable:
            return "Kagemusha ABI-7 artifacts are unavailable."
        }
    }
}

public enum OfflineKagemushaAbi7CapabilityContract {
    public static let mode = KagemushaOfflineSpendMode.recursiveCompactV1.rawValue
    public static let nativeBridgeAbiVersion = KagemushaRecursiveCompactPaymentTokenProver.requiredNativeBridgeAbiVersion
    public static let circuitId = KagemushaRecursiveCompactPaymentTokenProver.recursiveCompactCircuitIdV1

    public static func isSupported(
        offlinePayments: Bool,
        lifecycleEnabled: Bool,
        mode: String?,
        nativeBridgeAbiVersion: UInt32?,
        circuitId: String?,
        artifactSetId: String?,
        artifactsAvailable: Bool
    ) -> Bool {
        do {
            try validate(
                offlinePayments: offlinePayments,
                lifecycleEnabled: lifecycleEnabled,
                mode: mode,
                nativeBridgeAbiVersion: nativeBridgeAbiVersion,
                circuitId: circuitId,
                artifactSetId: artifactSetId,
                artifactsAvailable: artifactsAvailable
            )
            return true
        } catch {
            return false
        }
    }

    public static func validate(
        offlinePayments: Bool,
        lifecycleEnabled: Bool,
        mode: String?,
        nativeBridgeAbiVersion: UInt32?,
        circuitId: String?,
        artifactSetId: String?,
        artifactsAvailable: Bool
    ) throws {
        guard offlinePayments else {
            throw OfflineKagemushaAbi7CapabilityError.offlinePaymentsDisabled
        }
        guard lifecycleEnabled else {
            throw OfflineKagemushaAbi7CapabilityError.lifecycleDisabled
        }
        try validateArtifactMetadata(
            mode: mode,
            nativeBridgeAbiVersion: nativeBridgeAbiVersion,
            circuitId: circuitId,
            artifactSetId: artifactSetId
        )
        guard artifactsAvailable else {
            throw OfflineKagemushaAbi7CapabilityError.artifactsUnavailable
        }
    }

    public static func validateArtifactMetadata(
        mode: String?,
        nativeBridgeAbiVersion: UInt32?,
        circuitId: String?,
        artifactSetId: String?
    ) throws {
        guard mode == Self.mode else {
            throw OfflineKagemushaAbi7CapabilityError.unsupportedMode(expected: Self.mode, actual: mode)
        }
        guard nativeBridgeAbiVersion == Self.nativeBridgeAbiVersion else {
            throw OfflineKagemushaAbi7CapabilityError.unsupportedNativeBridgeAbi(
                expected: Self.nativeBridgeAbiVersion,
                actual: nativeBridgeAbiVersion
            )
        }
        guard circuitId == Self.circuitId else {
            throw OfflineKagemushaAbi7CapabilityError.unsupportedCircuitId(expected: Self.circuitId, actual: circuitId)
        }
        guard artifactSetId?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false else {
            throw OfflineKagemushaAbi7CapabilityError.missingArtifactSetId
        }
    }
}
