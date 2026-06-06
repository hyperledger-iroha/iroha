import Foundation

public enum KagemushaRecursiveCompactPaymentTokenProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case emptyPallasOpenEnvelopesArchive
    case invalidRecordBundleArchive
    case emptyRecordBundlePayload
    case invalidPallasOpenEnvelopesArchive
    case emptyPallasOpenEnvelopesPayload
    case emptyCompactTokenArchive
    case invalidCompactTokenArchive
    case emptyCompactTokenPayload
    case oversizedCompactTokenArchive
    case bridgeUnavailable
    case proofRejected
    case verificationRejected

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
        case .emptyCompactTokenArchive:
            return "Kagemusha recursive compact-token archive must not be empty."
        case .invalidCompactTokenArchive:
            return "Kagemusha recursive compact-token archive must be a valid Norito archive."
        case .emptyCompactTokenPayload:
            return "Kagemusha recursive compact-token archive must contain a non-empty Norito payload."
        case .oversizedCompactTokenArchive:
            return "Kagemusha recursive compact-token archive is oversized."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive compact-token prover/verifier is unavailable."
            )
        case .proofRejected:
            return "Kagemusha recursive compact-token inputs were rejected by the native prover."
        case .verificationRejected:
            return "Kagemusha recursive compact-token archive was rejected by the native verifier."
        }
    }
}

public enum KagemushaRecursiveCompactPaymentTokenProver {
    public static let requiredBridgeAbiVersion: UInt32 = 7
    public static let recursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1"
    private static let maxNoritoHeaderPaddingBytes = 64

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenProverAvailable
    }

    public static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data
    ) throws -> Data {
        try proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: recordBundleArchive,
            pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenProverAvailable
        ) {
            try NoritoNativeBridge.shared
                .proveKagemushaVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: recordBundleArchive,
                    pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive
                )
        }
    }

    public static func verifyRecursiveCompactPaymentToken(
        compactTokenArchive: Data
    ) throws -> Bool {
        try verifyRecursiveCompactPaymentToken(
            compactTokenArchive: compactTokenArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenProverAvailable
        ) {
            try NoritoNativeBridge.shared.verifyKagemushaRecursiveCompactPaymentToken(
                compactTokenArchive: compactTokenArchive
            )
        }
    }

    static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Data?
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyRecordBundleArchive
        }
        guard !pallasOpenEnvelopesArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyPallasOpenEnvelopesArchive
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
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        let token: Data?
        do {
            token = try body()
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveCompactPaymentTokenProverError.proofRejected
        } catch {
            throw KagemushaRecursiveCompactPaymentTokenProverError.proofRejected
        }
        guard let token else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        guard !token.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.proofRejected
        }
        try requireValidRecursiveCompactTokenArchive(token)
        return token
    }

    static func verifyRecursiveCompactPaymentToken(
        compactTokenArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Bool?
    ) throws -> Bool {
        guard !compactTokenArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyCompactTokenArchive
        }
        try requireValidRecursiveCompactTokenArchive(compactTokenArchive)
        guard bridgeAvailable else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        let valid: Bool?
        do {
            valid = try body()
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveCompactPaymentTokenProverError.verificationRejected
        } catch {
            throw KagemushaRecursiveCompactPaymentTokenProverError.verificationRejected
        }
        guard let valid else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        return valid
    }

    private static func requireValidInputArchive(
        _ archive: Data,
        invalidError: KagemushaRecursiveCompactPaymentTokenProverError,
        emptyPayloadError: KagemushaRecursiveCompactPaymentTokenProverError
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

    private static func requireValidRecursiveCompactTokenArchive(_ archive: Data) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.oversizedCompactTokenArchive
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.invalidCompactTokenArchive
        }
        guard frame.header.length > 0 else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyCompactTokenPayload
        }
    }
}
