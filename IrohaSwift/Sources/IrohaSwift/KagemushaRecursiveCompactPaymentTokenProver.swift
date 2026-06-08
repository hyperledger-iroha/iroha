import Foundation

public enum KagemushaRecursiveCompactPaymentTokenProverError: Error, Equatable, LocalizedError {
    case emptyRecordBundleArchive
    case emptyPallasOpenEnvelopesArchive
    case emptyKeyArtifactsArchive
    case emptyVerifierKeysArchive
    case oversizedRecordBundleArchive
    case oversizedPallasOpenEnvelopesArchive
    case oversizedKeyArtifactsArchive
    case oversizedVerifierKeysArchive
    case emptyBundleArchive
    case oversizedBundleArchive
    case invalidBundleArchive
    case emptyBundlePayload
    case invalidRecordBundleArchive
    case emptyRecordBundlePayload
    case invalidPallasOpenEnvelopesArchive
    case emptyPallasOpenEnvelopesPayload
    case invalidKeyArtifactsArchive
    case emptyKeyArtifactsPayload
    case invalidVerifierKeysArchive
    case emptyVerifierKeysPayload
    case emptyCompactTokenArchive
    case invalidCompactTokenArchive
    case emptyCompactTokenPayload
    case oversizedCompactTokenArchive
    case emptyVerifierRecordArchive
    case invalidVerifierRecordArchive
    case emptyVerifierRecordPayload
    case oversizedVerifierRecordArchive
    case bridgeUnavailable
    case recursiveCompactUnavailable
    case proofRejected
    case verificationRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not be empty."
        case .emptyPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must not be empty."
        case .emptyKeyArtifactsArchive:
            return "Kagemusha recursive compact key-artifacts archive must not be empty."
        case .emptyVerifierKeysArchive:
            return "Kagemusha recursive compact verifier-keys archive must not be empty."
        case .oversizedRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .oversizedPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .oversizedKeyArtifactsArchive:
            return "Kagemusha recursive compact key-artifacts archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .oversizedVerifierKeysArchive:
            return "Kagemusha recursive compact verifier-keys archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .emptyBundleArchive:
            return "Kagemusha recursive spend bundle archive must not be empty."
        case .oversizedBundleArchive:
            return "Kagemusha recursive spend bundle archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .invalidBundleArchive:
            return "Kagemusha recursive spend bundle archive must be a valid Norito archive."
        case .emptyBundlePayload:
            return "Kagemusha recursive spend bundle archive must contain a non-empty Norito payload."
        case .invalidRecordBundleArchive:
            return "Kagemusha verified fold record bundle archive must be a valid Norito archive."
        case .emptyRecordBundlePayload:
            return "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload."
        case .invalidPallasOpenEnvelopesArchive:
            return "Kagemusha Pallas open-envelope archive must be a valid Norito archive."
        case .emptyPallasOpenEnvelopesPayload:
            return "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload."
        case .invalidKeyArtifactsArchive:
            return "Kagemusha recursive compact key-artifacts archive must be a valid Norito archive."
        case .emptyKeyArtifactsPayload:
            return "Kagemusha recursive compact key-artifacts archive must contain a non-empty Norito payload."
        case .invalidVerifierKeysArchive:
            return "Kagemusha recursive compact verifier-keys archive must be a valid Norito archive."
        case .emptyVerifierKeysPayload:
            return "Kagemusha recursive compact verifier-keys archive must contain a non-empty Norito payload."
        case .emptyCompactTokenArchive:
            return "Kagemusha recursive compact-token archive must not be empty."
        case .invalidCompactTokenArchive:
            return "Kagemusha recursive compact-token archive must be a valid Norito archive."
        case .emptyCompactTokenPayload:
            return "Kagemusha recursive compact-token archive must contain a non-empty Norito payload."
        case .oversizedCompactTokenArchive:
            return "Kagemusha recursive compact-token archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .emptyVerifierRecordArchive:
            return "Kagemusha verifier record archive must not be empty."
        case .invalidVerifierRecordArchive:
            return "Kagemusha verifier record archive must be a valid Norito archive."
        case .emptyVerifierRecordPayload:
            return "Kagemusha verifier record archive must contain a non-empty Norito payload."
        case .oversizedVerifierRecordArchive:
            return "Kagemusha verifier record archive must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive compact-token prover/verifier is unavailable."
            )
        case .recursiveCompactUnavailable:
            return "Kagemusha recursive compact-token multi-hop proving is reserved until the append verifier batch is composed into the compact proof."
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

    public static var isVerifierNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenVerifierAvailable
    }

    public static var isProjectionNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable
    }

    public static var isProjectionVerifierNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable
    }

    public static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        recursiveCompactKeyArtifactsArchive: Data
    ) throws -> Data {
        try proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: recordBundleArchive,
            pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive,
            recursiveCompactKeyArtifactsArchive: recursiveCompactKeyArtifactsArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenProverAvailable
        ) {
            try NoritoNativeBridge.shared
                .proveKagemushaVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: recordBundleArchive,
                    pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive,
                    recursiveCompactKeyArtifactsArchive: recursiveCompactKeyArtifactsArchive
                )
        }
    }

    public static func verifyRecursiveCompactPaymentToken(
        compactTokenArchive: Data,
        recursiveCompactVerifierKeysArchive: Data
    ) throws -> Bool {
        try verifyRecursiveCompactPaymentToken(
            compactTokenArchive: compactTokenArchive,
            recursiveCompactVerifierKeysArchive: recursiveCompactVerifierKeysArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenVerifierAvailable
        ) {
            try NoritoNativeBridge.shared.verifyKagemushaRecursiveCompactPaymentToken(
                compactTokenArchive: compactTokenArchive,
                recursiveCompactVerifierKeysArchive: recursiveCompactVerifierKeysArchive
            )
        }
    }

    public static func recursiveSpendCompactPaymentTokenFromBundle(
        bundleArchive: Data
    ) throws -> Data {
        try recursiveSpendCompactPaymentTokenFromBundle(
            bundleArchive: bundleArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(
                bundleArchive: bundleArchive
            )
        }
    }

    public static func verifyRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive: Data,
        verifierRecordArchive: Data,
        blockHeight: UInt64? = nil
    ) throws -> Bool {
        try verifyRecursiveSpendCompactPaymentTokenProjection(
            compactTokenArchive: compactTokenArchive,
            verifierRecordArchive: verifierRecordArchive,
            bridgeAvailable: NoritoNativeBridge.shared
                .isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable
        ) {
            try NoritoNativeBridge.shared.verifyKagemushaRecursiveSpendCompactPaymentTokenProjection(
                compactTokenArchive: compactTokenArchive,
                verifierRecordArchive: verifierRecordArchive,
                blockHeight: blockHeight
            )
        }
    }

    static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        recursiveCompactKeyArtifactsArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Data?
    ) throws -> Data {
        guard !recordBundleArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyRecordBundleArchive
        }
        guard !pallasOpenEnvelopesArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyPallasOpenEnvelopesArchive
        }
        guard !recursiveCompactKeyArtifactsArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyKeyArtifactsArchive
        }
        try requireValidInputArchive(
            recordBundleArchive,
            oversizedError: .oversizedRecordBundleArchive,
            invalidError: .invalidRecordBundleArchive,
            emptyPayloadError: .emptyRecordBundlePayload
        )
        try requireValidInputArchive(
            pallasOpenEnvelopesArchive,
            oversizedError: .oversizedPallasOpenEnvelopesArchive,
            invalidError: .invalidPallasOpenEnvelopesArchive,
            emptyPayloadError: .emptyPallasOpenEnvelopesPayload
        )
        try requireValidInputArchive(
            recursiveCompactKeyArtifactsArchive,
            oversizedError: .oversizedKeyArtifactsArchive,
            invalidError: .invalidKeyArtifactsArchive,
            emptyPayloadError: .emptyKeyArtifactsPayload
        )
        guard bridgeAvailable else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        let token: Data?
        do {
            token = try body()
        } catch NativeBridgeError.kagemushaRecursiveCompactUnavailable {
            throw KagemushaRecursiveCompactPaymentTokenProverError.recursiveCompactUnavailable
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
        recursiveCompactVerifierKeysArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Bool?
    ) throws -> Bool {
        guard !compactTokenArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyCompactTokenArchive
        }
        guard !recursiveCompactVerifierKeysArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyVerifierKeysArchive
        }
        try requireValidRecursiveCompactTokenArchive(compactTokenArchive)
        try requireValidInputArchive(
            recursiveCompactVerifierKeysArchive,
            oversizedError: .oversizedVerifierKeysArchive,
            invalidError: .invalidVerifierKeysArchive,
            emptyPayloadError: .emptyVerifierKeysPayload
        )
        guard bridgeAvailable else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        let valid: Bool?
        do {
            valid = try body()
        } catch NativeBridgeError.kagemushaRecursiveCompactUnavailable {
            throw KagemushaRecursiveCompactPaymentTokenProverError.recursiveCompactUnavailable
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

    static func recursiveSpendCompactPaymentTokenFromBundle(
        bundleArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Data?
    ) throws -> Data {
        guard !bundleArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyBundleArchive
        }
        try requireValidInputArchive(
            bundleArchive,
            oversizedError: .oversizedBundleArchive,
            invalidError: .invalidBundleArchive,
            emptyPayloadError: .emptyBundlePayload
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

    static func verifyRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive: Data,
        verifierRecordArchive: Data,
        bridgeAvailable: Bool,
        body: () throws -> Bool?
    ) throws -> Bool {
        guard !compactTokenArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyCompactTokenArchive
        }
        guard !verifierRecordArchive.isEmpty else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.emptyVerifierRecordArchive
        }
        try requireValidRecursiveCompactTokenArchive(compactTokenArchive)
        try requireValidInputArchive(
            verifierRecordArchive,
            oversizedError: .oversizedVerifierRecordArchive,
            invalidError: .invalidVerifierRecordArchive,
            emptyPayloadError: .emptyVerifierRecordPayload
        )
        guard bridgeAvailable else {
            throw KagemushaRecursiveCompactPaymentTokenProverError.bridgeUnavailable
        }
        let valid: Bool?
        do {
            valid = try body()
        } catch NativeBridgeError.kagemushaRecursiveCompactUnavailable {
            throw KagemushaRecursiveCompactPaymentTokenProverError.recursiveCompactUnavailable
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
        oversizedError: KagemushaRecursiveCompactPaymentTokenProverError,
        invalidError: KagemushaRecursiveCompactPaymentTokenProverError,
        emptyPayloadError: KagemushaRecursiveCompactPaymentTokenProverError
    ) throws {
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes else {
            throw oversizedError
        }
        guard let frame = noritoDecodeFrame(archive),
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
