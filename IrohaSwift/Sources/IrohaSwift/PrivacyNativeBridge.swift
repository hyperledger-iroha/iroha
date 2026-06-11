import Foundation

public enum PrivacyNativeBridgeError: Error, Equatable, LocalizedError {
    case emptyRequestArchive
    case oversizedRequestArchive
    case bridgeUnavailable
    case nativeRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRequestArchive:
            return "Privacy Norito V1 request archive must not be empty."
        case .oversizedRequestArchive:
            return "Privacy Norito V1 request archive exceeds the native archive size limit."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Privacy native bridge is unavailable."
            )
        case .nativeRejected:
            return "Privacy native bridge returned no valid Norito V1 response."
        }
    }
}

public struct PrivacyProductionGate: Equatable, Sendable {
    public static let version = "privacy-production-gate-v1"
    public static let requiredGateKeys = [
        "real_proving",
        "real_verification",
        "chain_admission",
        "sdk_parity",
        "wallet_state",
        "witness_privacy_checks",
        "deterministic_tests",
        "negative_adversarial_tests",
        "replay_nullifier_tests",
        "fuzzing",
        "parser_fuzzing",
        "verifier_fuzzing",
        "performance_gates",
        "external_audit"
    ]
    public static let missingReasons = [
        "real proving engine is not registered",
        "real verifier is not registered",
        "chain admission path is not enabled",
        "cross-SDK parity is incomplete",
        "wallet/state support is incomplete",
        "witness privacy checks are incomplete",
        "deterministic tests are incomplete",
        "negative/adversarial tests are incomplete",
        "replay/nullifier rejection tests are incomplete",
        "fuzzing gate is incomplete",
        "parser fuzzing gate is incomplete",
        "verifier fuzzing gate is incomplete",
        "performance gate is incomplete",
        "internal cryptographic review signoff is missing",
        "implementation stage is not production-hardened",
        "planned SDK entrypoints remain",
        "dev fixture entrypoints are not production entrypoints",
        "Iroha production allowlist is not enabled for this audited row"
    ]

    public let version: String
    public let ready: Bool
    public let realProving: Bool
    public let realVerification: Bool
    public let chainAdmission: Bool
    public let sdkParity: Bool
    public let walletState: Bool
    public let witnessPrivacyChecks: Bool
    public let deterministicTests: Bool
    public let negativeAdversarialTests: Bool
    public let replayNullifierTests: Bool
    public let fuzzing: Bool
    public let parserFuzzing: Bool
    public let verifierFuzzing: Bool
    public let performanceGates: Bool
    public let externalAudit: Bool
    public let requiredGates: [String]
    public let missing: [String]
    public let auditReferences: [String]

    public static var failClosed: PrivacyProductionGate {
        PrivacyProductionGate()
    }

    private init() {
        version = Self.version
        ready = false
        realProving = false
        realVerification = false
        chainAdmission = false
        sdkParity = false
        walletState = false
        witnessPrivacyChecks = false
        deterministicTests = false
        negativeAdversarialTests = false
        replayNullifierTests = false
        fuzzing = false
        parserFuzzing = false
        verifierFuzzing = false
        performanceGates = false
        externalAudit = false
        requiredGates = Self.requiredGateKeys
        missing = Self.missingReasons
        auditReferences = []
    }
}

public struct PrivacyCapabilities: Equatable, Sendable {
    public let swiftSdkAvailable: Bool
    public let bridgeAvailable: Bool
    public let productionReady: Bool
    public let productionGate: PrivacyProductionGate

    public init(bridgeAvailable: Bool) {
        swiftSdkAvailable = true
        self.bridgeAvailable = bridgeAvailable
        productionReady = false
        productionGate = .failClosed
    }
}

public enum PrivacyNativeBridge {
    public static let ffiVersionV1: UInt32 = 1
    public static let requiredBridgeAbiVersion: UInt32 = 7
    public static let privacyNativeArchiveMaxBytes = 64 * 1024 * 1024
    public static let ffiStatusError: UInt32 = 1
    public static let ffiErrorNullPointer: UInt32 = 1
    public static let ffiErrorMalformedNorito: UInt32 = 2
    public static let ffiErrorUnsupportedAlgorithm: UInt32 = 3
    public static let ffiErrorProductionDisabled: UInt32 = 4
    public static let ffiErrorInvalidRequest: UInt32 = 5
    static let privacyRequestSchemaByte: UInt8 = 0x52
    static let privacyCapabilitiesResultSchemaByte: UInt8 = 0x50
    static let privacyBuildProofResultSchemaByte: UInt8 = 0x42
    static let privacyVerifyProofResultSchemaByte: UInt8 = 0x56

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isPrivacyNativeAvailable
    }

    public static func privacyCapabilities(
        bridgeAvailable: Bool = NoritoNativeBridge.shared.isPrivacyNativeAvailable
    ) -> PrivacyCapabilities {
        PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
    }

    public static func capabilitiesV1() throws -> Data {
        try call(
            bridgeAvailable: NoritoNativeBridge.shared.isPrivacyNativeAvailable,
            expectedSchemaByte: privacyCapabilitiesResultSchemaByte
        ) {
            try NoritoNativeBridge.shared.privacyCapabilitiesV1()
        }
    }

    public static func privacyProofRequestV1(
        algorithmId: String,
        entrypoint: String,
        vkRef: String,
        publicInputs: Data,
        witness: Data = Data(),
        proof: Data = Data()
    ) throws -> Data {
        try call(
            bridgeAvailable: NoritoNativeBridge.shared.isPrivacyNativeAvailable,
            expectedSchemaByte: privacyRequestSchemaByte
        ) {
            try NoritoNativeBridge.shared.privacyProofRequestV1(
                algorithmId: algorithmId,
                entrypoint: entrypoint,
                vkRef: vkRef,
                publicInputs: publicInputs,
                witness: witness,
                proof: proof
            )
        }
    }

    public static func buildProofV1(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isPrivacyNativeAvailable,
            expectedSchemaByte: privacyBuildProofResultSchemaByte
        ) {
            try NoritoNativeBridge.shared.privacyBuildProofV1(requestArchive: $0)
        }
    }

    public static func buildConfidentialTransferProofV2(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildConfidentialUnshieldProofV3(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildZkAceAuthorizationProofV1(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildJindoLatticeProofV0(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildSisHintsAnonymousCredentialProofV0(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildSilentThresholdCredentialShowingProofV0(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildVegaCredentialPredicateProofV0(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildZkAmsAdmissionBatchProofV0(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func buildZkAtPolicyProofV1(requestArchive: Data) throws -> Data {
        try buildProofV1(requestArchive: requestArchive)
    }

    public static func verifyProofV1(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isPrivacyNativeAvailable,
            expectedSchemaByte: privacyVerifyProofResultSchemaByte
        ) {
            try NoritoNativeBridge.shared.privacyVerifyProofV1(requestArchive: $0)
        }
    }

    public static func verifyJindoPolynomialCommitmentV0(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    public static func verifySisHintsAnonymousCredentialProofV0(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    public static func verifySilentThresholdCredentialShowingProofV0(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    public static func verifyVegaCredentialPredicateProofV0(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    public static func verifyZkAmsAdmissionBatchProofV0(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    public static func verifyZkAtPolicyProofV1(requestArchive: Data) throws -> Data {
        try verifyProofV1(requestArchive: requestArchive)
    }

    static func call(
        requestArchive: Data,
        bridgeAvailable: Bool,
        expectedSchemaByte: UInt8,
        _ body: (Data) throws -> Data?
    ) throws -> Data {
        guard !requestArchive.isEmpty else {
            throw PrivacyNativeBridgeError.emptyRequestArchive
        }
        guard requestArchive.count <= privacyNativeArchiveMaxBytes else {
            throw PrivacyNativeBridgeError.oversizedRequestArchive
        }
        guard NoritoNativeBridge.isValidPrivacyNoritoArchive(requestArchive) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard hasPrivacyNoritoSchema(
            requestArchive,
            expectedSchemaByte: privacyRequestSchemaByte
        ) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard NoritoNativeBridge.hasNonEmptyPrivacyNoritoPayload(requestArchive) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        return try call(
            bridgeAvailable: bridgeAvailable,
            expectedSchemaByte: expectedSchemaByte
        ) {
            try body(requestArchive)
        }
    }

    static func call(
        bridgeAvailable: Bool,
        expectedSchemaByte: UInt8,
        _ body: () throws -> Data?
    ) throws -> Data {
        guard bridgeAvailable else {
            throw PrivacyNativeBridgeError.bridgeUnavailable
        }
        let archive: Data?
        do {
            archive = try body()
        } catch {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard let archive else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard !archive.isEmpty else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard archive.count <= privacyNativeArchiveMaxBytes else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard NoritoNativeBridge.isValidPrivacyNoritoArchive(archive) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard NoritoNativeBridge.hasNonEmptyPrivacyNoritoPayload(archive) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        guard hasPrivacyNoritoSchema(archive, expectedSchemaByte: expectedSchemaByte) else {
            throw PrivacyNativeBridgeError.nativeRejected
        }
        return archive
    }

    private static func hasPrivacyNoritoSchema(
        _ archive: Data,
        expectedSchemaByte: UInt8
    ) -> Bool {
        guard archive.count >= 22 else {
            return false
        }
        return archive[6..<22].allSatisfy { $0 == expectedSchemaByte }
    }
}
