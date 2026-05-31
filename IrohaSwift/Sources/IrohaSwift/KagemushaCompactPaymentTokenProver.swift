import Foundation

public enum KagemushaCompactPaymentTokenProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case bridgeUnavailable
    case proofRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not be empty."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha compact-token prover is unavailable."
            )
        case .proofRejected:
            return "Kagemusha verified fold record bundle was rejected by the native prover."
        }
    }
}

public enum KagemushaCompactPaymentTokenProver {
    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaCompactPaymentTokenProverAvailable
    }

    public static func proveVerifiedCompactPaymentTokenWithRecords(
        recordBundleArchive: Data
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaCompactPaymentTokenProverError.emptyRecordBundleArchive
        }
        guard NoritoNativeBridge.shared.isKagemushaCompactPaymentTokenProverAvailable else {
            throw KagemushaCompactPaymentTokenProverError.bridgeUnavailable
        }
        do {
            guard let token = try NoritoNativeBridge.shared
                .proveKagemushaVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: recordBundleArchive
                )
            else {
                throw KagemushaCompactPaymentTokenProverError.bridgeUnavailable
            }
            return token
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaCompactPaymentTokenProverError.proofRejected
        } catch {
            throw KagemushaCompactPaymentTokenProverError.proofRejected
        }
    }
}
