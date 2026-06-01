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
        try proveVerifiedCompactPaymentTokenWithRecords(
            recordBundleArchive: recordBundleArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaCompactPaymentTokenProverAvailable
        ) {
            try NoritoNativeBridge.shared
                .proveKagemushaVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: recordBundleArchive
                )
        }
    }

    static func proveVerifiedCompactPaymentTokenWithRecords(
        recordBundleArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Data?
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaCompactPaymentTokenProverError.emptyRecordBundleArchive
        }
        guard bridgeAvailable else {
            throw KagemushaCompactPaymentTokenProverError.bridgeUnavailable
        }
        let token: Data?
        do {
            token = try body()
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaCompactPaymentTokenProverError.proofRejected
        } catch {
            throw KagemushaCompactPaymentTokenProverError.proofRejected
        }
        guard let token else {
            throw KagemushaCompactPaymentTokenProverError.bridgeUnavailable
        }
        guard !token.isEmpty else {
            throw KagemushaCompactPaymentTokenProverError.proofRejected
        }
        return token
    }
}
