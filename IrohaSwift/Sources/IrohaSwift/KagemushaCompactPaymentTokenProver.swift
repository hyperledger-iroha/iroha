import Foundation

public enum KagemushaCompactPaymentTokenProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case invalidRecordBundleArchive
    case emptyRecordBundlePayload
    case oversizedCompactTokenArchive
    case invalidCompactTokenArchive
    case emptyCompactTokenPayload
    case bridgeUnavailable
    case proofRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not be empty."
        case .invalidRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must be a valid Norito archive."
        case .emptyRecordBundlePayload:
            return "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload."
        case .oversizedCompactTokenArchive:
            return "Kagemusha compact-token native bridge returned an oversized archive."
        case .invalidCompactTokenArchive:
            return "Kagemusha compact-token native bridge returned an invalid Norito archive."
        case .emptyCompactTokenPayload:
            return "Kagemusha compact-token native bridge returned an empty Norito payload."
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
    private static let maxNoritoHeaderPaddingBytes = 64

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
        try requireValidRecordBundleArchive(recordBundleArchive)
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
        try requireValidCompactTokenArchive(token)
        return token
    }

    private static func requireValidRecordBundleArchive(_ archive: Data) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw KagemushaCompactPaymentTokenProverError.invalidRecordBundleArchive
        }
        guard frame.header.length > 0 else {
            throw KagemushaCompactPaymentTokenProverError.emptyRecordBundlePayload
        }
    }

    private static func requireValidCompactTokenArchive(_ archive: Data) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes else {
            throw KagemushaCompactPaymentTokenProverError.oversizedCompactTokenArchive
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw KagemushaCompactPaymentTokenProverError.invalidCompactTokenArchive
        }
        guard frame.header.length > 0 else {
            throw KagemushaCompactPaymentTokenProverError.emptyCompactTokenPayload
        }
    }
}
