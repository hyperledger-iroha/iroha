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

    fileprivate static func fromNativeRows(
        _ rows: [NativePrivacyCapability]
    ) -> PrivacyProductionGate {
        if rows.isEmpty || rows.contains(where: { !nativeCapabilityRowIsExact($0) }) {
            return .failClosed
        }

        let ready = rows.allSatisfy { row in
            row.productionReady &&
                row.plannedEntrypoints.isEmpty &&
                row.productionGate.ready
        }
        let auditReferences = stableDistinct(
            rows.flatMap { $0.productionGate.auditReferences }
        )
        let aggregateReady = ready &&
            requiredGateKeys.allSatisfy { nativeGatePassed(rows, key: $0) } &&
            !auditReferences.isEmpty
        let missing = stableDistinct(rows.flatMap { $0.productionGate.missing })

        return PrivacyProductionGate(
            version: version,
            ready: aggregateReady,
            realProving: nativeGatePassed(rows, key: "real_proving"),
            realVerification: nativeGatePassed(rows, key: "real_verification"),
            chainAdmission: nativeGatePassed(rows, key: "chain_admission"),
            sdkParity: nativeGatePassed(rows, key: "sdk_parity"),
            walletState: nativeGatePassed(rows, key: "wallet_state"),
            witnessPrivacyChecks: nativeGatePassed(rows, key: "witness_privacy_checks"),
            deterministicTests: nativeGatePassed(rows, key: "deterministic_tests"),
            negativeAdversarialTests: nativeGatePassed(rows, key: "negative_adversarial_tests"),
            replayNullifierTests: nativeGatePassed(rows, key: "replay_nullifier_tests"),
            fuzzing: nativeGatePassed(rows, key: "fuzzing"),
            parserFuzzing: nativeGatePassed(rows, key: "parser_fuzzing"),
            verifierFuzzing: nativeGatePassed(rows, key: "verifier_fuzzing"),
            performanceGates: nativeGatePassed(rows, key: "performance_gates"),
            externalAudit: nativeGatePassed(rows, key: "external_audit"),
            requiredGates: requiredGateKeys,
            missing: aggregateReady ? [] : stableDistinct(missing.isEmpty ? missingReasons : missing),
            auditReferences: stableDistinct(auditReferences)
        )
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

    private init(
        version: String,
        ready: Bool,
        realProving: Bool,
        realVerification: Bool,
        chainAdmission: Bool,
        sdkParity: Bool,
        walletState: Bool,
        witnessPrivacyChecks: Bool,
        deterministicTests: Bool,
        negativeAdversarialTests: Bool,
        replayNullifierTests: Bool,
        fuzzing: Bool,
        parserFuzzing: Bool,
        verifierFuzzing: Bool,
        performanceGates: Bool,
        externalAudit: Bool,
        requiredGates: [String],
        missing: [String],
        auditReferences: [String]
    ) {
        self.version = version
        self.ready = ready
        self.realProving = realProving
        self.realVerification = realVerification
        self.chainAdmission = chainAdmission
        self.sdkParity = sdkParity
        self.walletState = walletState
        self.witnessPrivacyChecks = witnessPrivacyChecks
        self.deterministicTests = deterministicTests
        self.negativeAdversarialTests = negativeAdversarialTests
        self.replayNullifierTests = replayNullifierTests
        self.fuzzing = fuzzing
        self.parserFuzzing = parserFuzzing
        self.verifierFuzzing = verifierFuzzing
        self.performanceGates = performanceGates
        self.externalAudit = externalAudit
        self.requiredGates = requiredGates
        self.missing = missing
        self.auditReferences = auditReferences
    }

    private static let readyAuditReferencePrefixes = [
        "chain_id:",
        "reviewer:",
        "review_artifact_hash:",
        "review_artifact_signature:",
        "fuzz_artifact_hash:",
        "performance_artifact_hash:",
        "localnet_run_id:",
        "localnet_smoke_tx_hash:",
        "localnet_replay_rejection_hash:",
        "localnet_restart_replay_rejection_hash:",
        "localnet_state_recovery_hash:",
        "localnet_lifecycle_shield_tx_hash:",
        "localnet_lifecycle_hop_proof_hash:",
        "localnet_lifecycle_recursive_init_hash:",
        "localnet_lifecycle_recursive_init_verify_hash:",
        "localnet_lifecycle_recursive_append_hash:",
        "localnet_lifecycle_recursive_append_verify_hash:",
        "localnet_lifecycle_unshield_proof_hash:",
        "localnet_lifecycle_redeem_tx_hash:"
    ]

    private static let readyHashReferencePrefixes = Set(
        readyAuditReferencePrefixes.filter {
            $0.hasSuffix("_hash:") ||
                $0.hasSuffix("_tx_hash:") ||
                $0.hasSuffix("_proof_hash:")
        }
    )

    private static func nativeGatePassed(
        _ rows: [NativePrivacyCapability],
        key: String
    ) -> Bool {
        rows.allSatisfy { row in
            !row.productionGate.requiredGates.contains(key) ||
                row.productionGate.gates.contains { status in
                    status.key == key && status.passed
                }
        }
    }

    private static func nativeCapabilityRowIsExact(
        _ row: NativePrivacyCapability
    ) -> Bool {
        let gate = row.productionGate
        if gate.version != version ||
            row.productionReady != gate.ready ||
            gate.requiredGates != requiredGateKeys ||
            gate.gates.map(\.key) != requiredGateKeys ||
            gate.gates.contains(where: { $0.passed != gate.ready }) {
            return false
        }

        if gate.ready {
            return row.plannedEntrypoints.isEmpty &&
                gate.missing.isEmpty &&
                readyAuditReferencesAreExact(gate.auditReferences)
        }
        return gate.auditReferences.isEmpty && !gate.missing.isEmpty
    }

    private static func readyAuditReferencesAreExact(_ references: [String]) -> Bool {
        if references.count != readyAuditReferencePrefixes.count ||
            Set(references).count != references.count {
            return false
        }

        return zip(references, readyAuditReferencePrefixes).allSatisfy { reference, prefix in
            guard reference.hasPrefix(prefix),
                  productionEvidenceTextIsClean(reference) else {
                return false
            }
            let value = String(reference.dropFirst(prefix.count))
            if prefix == "review_artifact_signature:" {
                return productionSignatureIsValid(value)
            }
            if readyHashReferencePrefixes.contains(prefix) {
                return productionHashIsValid(value)
            }
            return true
        }
    }

    private static func productionHashIsValid(_ value: String) -> Bool {
        let prefix = "sha256:"
        guard value.hasPrefix(prefix), value.count == prefix.count + 64 else {
            return false
        }
        return value.dropFirst(prefix.count).unicodeScalars.allSatisfy {
            ($0.value >= 0x30 && $0.value <= 0x39) ||
                ($0.value >= 0x61 && $0.value <= 0x66)
        }
    }

    private static func productionSignatureIsValid(_ value: String) -> Bool {
        let prefix = "ed25519:"
        guard value.hasPrefix(prefix), value.count == prefix.count + 128 else {
            return false
        }
        return value.dropFirst(prefix.count).unicodeScalars.allSatisfy {
            ($0.value >= 0x30 && $0.value <= 0x39) ||
                ($0.value >= 0x61 && $0.value <= 0x66)
        }
    }

    private static func productionEvidenceTextIsClean(_ value: String) -> Bool {
        if value.isEmpty ||
            value.count > 768 ||
            value.trimmingCharacters(in: .whitespacesAndNewlines) != value {
            return false
        }

        var compact = ""
        for scalar in value.unicodeScalars {
            let code = scalar.value
            if code < 0x20 || code > 0x7E || code == 0x5C {
                return false
            }
            if (code >= 0x30 && code <= 0x39) ||
                (code >= 0x41 && code <= 0x5A) ||
                (code >= 0x61 && code <= 0x7A) {
                compact.append(String(scalar).lowercased())
            }
        }

        return !compact.contains("devfixture") &&
            !compact.contains("devprooffixture") &&
            !compact.contains("localonly") &&
            !compact.contains("mock")
    }

    private static func stableDistinct(_ values: [String]) -> [String] {
        var seen = Set<String>()
        var result: [String] = []
        for value in values where !seen.contains(value) {
            seen.insert(value)
            result.append(value)
        }
        return result
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

    private init(
        bridgeAvailable: Bool,
        productionReady: Bool,
        productionGate: PrivacyProductionGate
    ) {
        swiftSdkAvailable = true
        self.bridgeAvailable = bridgeAvailable
        self.productionReady = productionReady
        self.productionGate = productionGate
    }

    fileprivate static func fromNative(
        _ native: NativePrivacyCapabilities,
        bridgeAvailable: Bool
    ) -> PrivacyCapabilities {
        if native.version != PrivacyNativeBridge.ffiVersionV1 ||
            native.gateVersion != PrivacyProductionGate.version {
            return PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
        }

        var algorithmsById: [String: NativePrivacyCapability] = [:]
        for row in native.algorithms {
            if algorithmsById[row.algorithmId] != nil {
                return PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
            }
            algorithmsById[row.algorithmId] = row
        }
        guard let confidentialTransfer = algorithmsById["confidential-transfer-v2"],
              let unshield = algorithmsById["unshield"] else {
            return PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
        }

        let productionGate = PrivacyProductionGate.fromNativeRows([
            confidentialTransfer,
            unshield
        ])
        return PrivacyCapabilities(
            bridgeAvailable: bridgeAvailable,
            productionReady: productionGate.ready,
            productionGate: productionGate
        )
    }
}

fileprivate struct NativePrivacyGateStatus {
    let key: String
    let passed: Bool
}

fileprivate struct NativePrivacyProductionGate {
    let version: String
    let ready: Bool
    let gates: [NativePrivacyGateStatus]
    let requiredGates: [String]
    let missing: [String]
    let auditReferences: [String]
}

fileprivate struct NativePrivacyCapability {
    let algorithmId: String
    let proofFamily: String
    let backendFamily: String
    let sdkEntrypoints: [String]
    let plannedEntrypoints: [String]
    let productionReady: Bool
    let productionGate: NativePrivacyProductionGate
}

fileprivate struct NativePrivacyCapabilities {
    let version: UInt32
    let gateVersion: String
    let algorithms: [NativePrivacyCapability]
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
        guard bridgeAvailable else {
            return PrivacyCapabilities(bridgeAvailable: false)
        }
        guard let archive = try? capabilitiesV1() else {
            return PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
        }
        return privacyCapabilities(fromArchive: archive, bridgeAvailable: bridgeAvailable)
    }

    static func privacyCapabilities(
        fromArchive archive: Data,
        bridgeAvailable: Bool = true
    ) -> PrivacyCapabilities {
        do {
            let native = try decodeNativeCapabilitiesArchive(archive)
            return PrivacyCapabilities.fromNative(native, bridgeAvailable: bridgeAvailable)
        } catch {
            return PrivacyCapabilities(bridgeAvailable: bridgeAvailable)
        }
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

    private struct NativeNoritoDecodeContext {
        let flags: UInt8

        var compactLenActive: Bool {
            (flags & NoritoHeader.compactLen) != 0
        }

        var packedSeqActive: Bool {
            (flags & NoritoHeader.packedSeq) != 0
        }

        var packedStructWithFieldBitset: Bool {
            (flags & NoritoHeader.packedStruct) != 0 &&
                (flags & NoritoHeader.fieldBitset) != 0
        }
    }

    private static func decodeNativeCapabilitiesArchive(
        _ archive: Data
    ) throws -> NativePrivacyCapabilities {
        guard let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.schema == [UInt8](
                repeating: privacyCapabilitiesResultSchemaByte,
                count: 16
              ) else {
            throw OfflineNoritoDecodingError.invalidField("invalid privacy capabilities archive")
        }
        let context = NativeNoritoDecodeContext(flags: frame.header.flags)
        var reader = OfflineNoritoReader(data: frame.payload)
        let capabilities = try decodeNativeCapabilities(&reader, context: context)
        try requireNativeFullyRead(reader, label: "native capabilities archive")
        return capabilities
    }

    private static func decodeNativeCapabilities(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext
    ) throws -> NativePrivacyCapabilities {
        let packedSizes = try readNativePackedFieldSizes(
            &reader,
            fieldCount: 3,
            context: context,
            label: "NativeCapabilities"
        )
        let version = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 0,
            label: "NativeCapabilities.version"
        ) { fieldReader in
            try fieldReader.readUInt32LE()
        }
        let gateVersion = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 1,
            label: "NativeCapabilities.gate_version"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let algorithms = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 2,
            label: "NativeCapabilities.algorithms"
        ) { fieldReader in
            try readNativeSequence(
                &fieldReader,
                context: context,
                label: "NativeCapabilities.algorithms"
            ) { elementReader in
                try decodeNativeCapability(&elementReader, context: context)
            }
        }
        return NativePrivacyCapabilities(
            version: version,
            gateVersion: gateVersion,
            algorithms: algorithms
        )
    }

    private static func decodeNativeCapability(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext
    ) throws -> NativePrivacyCapability {
        let packedSizes = try readNativePackedFieldSizes(
            &reader,
            fieldCount: 7,
            context: context,
            label: "NativeCapability"
        )
        let algorithmId = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 0,
            label: "NativeCapability.algorithm_id"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let proofFamily = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 1,
            label: "NativeCapability.proof_family"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let backendFamily = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 2,
            label: "NativeCapability.backend_family"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let sdkEntrypoints = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 3,
            label: "NativeCapability.sdk_entrypoints"
        ) { fieldReader in
            try readNativeStringSequence(
                &fieldReader,
                context: context,
                label: "NativeCapability.sdk_entrypoints"
            )
        }
        let plannedEntrypoints = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 4,
            label: "NativeCapability.planned_entrypoints"
        ) { fieldReader in
            try readNativeStringSequence(
                &fieldReader,
                context: context,
                label: "NativeCapability.planned_entrypoints"
            )
        }
        let productionReady = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 5,
            label: "NativeCapability.production_ready"
        ) { fieldReader in
            try readNativeBool(&fieldReader)
        }
        let productionGate = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 6,
            label: "NativeCapability.production_gate"
        ) { fieldReader in
            try decodeNativeProductionGate(&fieldReader, context: context)
        }
        return NativePrivacyCapability(
            algorithmId: algorithmId,
            proofFamily: proofFamily,
            backendFamily: backendFamily,
            sdkEntrypoints: sdkEntrypoints,
            plannedEntrypoints: plannedEntrypoints,
            productionReady: productionReady,
            productionGate: productionGate
        )
    }

    private static func decodeNativeProductionGate(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext
    ) throws -> NativePrivacyProductionGate {
        let packedSizes = try readNativePackedFieldSizes(
            &reader,
            fieldCount: 6,
            context: context,
            label: "NativeProductionGate"
        )
        let version = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 0,
            label: "NativeProductionGate.version"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let ready = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 1,
            label: "NativeProductionGate.ready"
        ) { fieldReader in
            try readNativeBool(&fieldReader)
        }
        let gates = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 2,
            label: "NativeProductionGate.gates"
        ) { fieldReader in
            try readNativeSequence(
                &fieldReader,
                context: context,
                label: "NativeProductionGate.gates"
            ) { elementReader in
                try decodeNativeGateStatus(&elementReader, context: context)
            }
        }
        let requiredGates = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 3,
            label: "NativeProductionGate.required_gates"
        ) { fieldReader in
            try readNativeStringSequence(
                &fieldReader,
                context: context,
                label: "NativeProductionGate.required_gates"
            )
        }
        let missing = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 4,
            label: "NativeProductionGate.missing"
        ) { fieldReader in
            try readNativeStringSequence(
                &fieldReader,
                context: context,
                label: "NativeProductionGate.missing"
            )
        }
        let auditReferences = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 5,
            label: "NativeProductionGate.audit_references"
        ) { fieldReader in
            try readNativeStringSequence(
                &fieldReader,
                context: context,
                label: "NativeProductionGate.audit_references"
            )
        }
        return NativePrivacyProductionGate(
            version: version,
            ready: ready,
            gates: gates,
            requiredGates: requiredGates,
            missing: missing,
            auditReferences: auditReferences
        )
    }

    private static func decodeNativeGateStatus(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext
    ) throws -> NativePrivacyGateStatus {
        let packedSizes = try readNativePackedFieldSizes(
            &reader,
            fieldCount: 2,
            context: context,
            label: "NativeGateStatus"
        )
        let key = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 0,
            label: "NativeGateStatus.key"
        ) { fieldReader in
            try readNativeString(&fieldReader, context: context)
        }
        let passed = try decodeNativeStructField(
            &reader,
            packedSizes: packedSizes,
            fieldIndex: 1,
            label: "NativeGateStatus.passed"
        ) { fieldReader in
            try readNativeBool(&fieldReader)
        }
        return NativePrivacyGateStatus(key: key, passed: passed)
    }

    private static func readNativeStringSequence(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext,
        label: String
    ) throws -> [String] {
        try readNativeSequence(&reader, context: context, label: label) { elementReader in
            try readNativeString(&elementReader, context: context)
        }
    }

    private static func readNativeSequence<T>(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext,
        label: String,
        decodeElement: (inout OfflineNoritoReader) throws -> T
    ) throws -> [T] {
        let count = try readNativeLength(&reader, compact: false)
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("\(label) length overflow")
        }
        if count > 0 && count > UInt64(reader.remaining()) {
            throw OfflineNoritoDecodingError.invalidField("\(label) length exceeds payload")
        }

        let elementCount = Int(count)
        var values: [T] = []
        values.reserveCapacity(elementCount)

        if context.packedSeqActive {
            if elementCount == 0 {
                guard reader.remaining() == 0 || reader.remaining() >= 8 else {
                    throw OfflineNoritoDecodingError.invalidField("\(label) packed zero length")
                }
                if reader.remaining() >= 8 {
                    let prefix = try reader.readBytes(8)
                    guard prefix.allSatisfy({ $0 == 0 }) else {
                        throw OfflineNoritoDecodingError.invalidField(
                            "\(label) packed zero offsets"
                        )
                    }
                }
                return values
            }

            var previous = try reader.readUInt64LE()
            guard previous == 0 else {
                throw OfflineNoritoDecodingError.invalidField("\(label) offsets start")
            }
            var sizes: [Int] = []
            sizes.reserveCapacity(elementCount)
            for _ in 0..<elementCount {
                let current = try reader.readUInt64LE()
                guard current >= previous,
                      current - previous <= UInt64(Int.max) else {
                    throw OfflineNoritoDecodingError.invalidField("\(label) offsets")
                }
                sizes.append(Int(current - previous))
                previous = current
            }
            for size in sizes {
                var child = OfflineNoritoReader(data: try reader.readBytes(size))
                let value = try decodeElement(&child)
                try requireNativeFullyRead(child, label: "\(label) element")
                values.append(value)
            }
            return values
        }

        for _ in 0..<elementCount {
            let length = try readNativeLength(&reader, compact: context.compactLenActive)
            guard length <= UInt64(Int.max) else {
                throw OfflineNoritoDecodingError.invalidField("\(label) element length overflow")
            }
            var child = OfflineNoritoReader(data: try reader.readBytes(Int(length)))
            let value = try decodeElement(&child)
            try requireNativeFullyRead(child, label: "\(label) element")
            values.append(value)
        }
        return values
    }

    private static func readNativeString(
        _ reader: inout OfflineNoritoReader,
        context: NativeNoritoDecodeContext
    ) throws -> String {
        let length = try readNativeLength(&reader, compact: context.compactLenActive)
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8")
        }
        return value
    }

    private static func readNativeBool(_ reader: inout OfflineNoritoReader) throws -> Bool {
        switch try reader.readUInt8() {
        case 0:
            return false
        case 1:
            return true
        default:
            throw OfflineNoritoDecodingError.invalidField("invalid native boolean")
        }
    }

    private static func readNativeLength(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> UInt64 {
        compact ? try reader.readVarint() : try reader.readUInt64LE()
    }

    private static func readNativePackedFieldSizes(
        _ reader: inout OfflineNoritoReader,
        fieldCount: Int,
        context: NativeNoritoDecodeContext,
        label: String
    ) throws -> [Int?]? {
        guard context.packedStructWithFieldBitset else {
            return nil
        }
        let bitsetBytes = (fieldCount + 7) / 8
        let bitsetData = try reader.readBytes(bitsetBytes)
        var bitset: UInt64 = 0
        for index in 0..<bitsetBytes {
            bitset |= UInt64(bitsetData[index]) << UInt64(index * 8)
        }
        for bit in fieldCount..<(bitsetBytes * 8) where ((bitset >> UInt64(bit)) & 1) != 0 {
            throw OfflineNoritoDecodingError.invalidField("\(label) unused field bit")
        }

        var sizes = Array<Int?>(repeating: nil, count: fieldCount)
        for index in 0..<fieldCount where ((bitset >> UInt64(index)) & 1) != 0 {
            let size = try reader.readVarint()
            guard size <= UInt64(Int.max) else {
                throw OfflineNoritoDecodingError.invalidField("\(label) packed field too large")
            }
            sizes[index] = Int(size)
        }
        return sizes
    }

    private static func decodeNativeStructField<T>(
        _ reader: inout OfflineNoritoReader,
        packedSizes: [Int?]?,
        fieldIndex: Int,
        label: String,
        decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        if let packedSizes, let size = packedSizes[fieldIndex] {
            var child = OfflineNoritoReader(data: try reader.readBytes(size))
            let value = try decode(&child)
            try requireNativeFullyRead(child, label: label)
            return value
        }
        return try decode(&reader)
    }

    private static func requireNativeFullyRead(
        _ reader: OfflineNoritoReader,
        label: String
    ) throws {
        guard reader.remaining() == 0 else {
            throw OfflineNoritoDecodingError.invalidField("\(label) trailing bytes")
        }
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
