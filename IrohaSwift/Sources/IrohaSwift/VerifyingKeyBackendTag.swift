import Foundation

/// Low-level proof engines accepted by the canonical
/// `iroha_data_model::zk::BackendTag` Norito wire contract.
///
/// Privacy protocols and verifier profiles are deliberately represented by
/// separate catalog labels and never become wire-enum variants.
public enum VerifyingKeyBackendTag: UInt32, CaseIterable, Sendable, Equatable {
    case halo2IpaPasta = 0
    case stark = 1

    public var noritoDiscriminant: UInt32 { rawValue }

    public var canonicalLabel: String {
        switch self {
        case .halo2IpaPasta:
            return "halo2-ipa-pasta"
        case .stark:
            return "stark"
        }
    }

    /// Parse one exact canonical Norito label.
    ///
    /// No whitespace, case, punctuation, or historical aliases are accepted.
    public init?(canonicalLabel: String) {
        switch canonicalLabel {
        case "halo2-ipa-pasta":
            self = .halo2IpaPasta
        case "stark":
            self = .stark
        default:
            return nil
        }
    }

    /// Returns whether a catalog label is known but still blocked from
    /// production verifier admission.
    public static func isPendingProductionBackendLabel(_ raw: String) -> Bool {
        VerifierBackendCatalogTag(catalogLabel: raw).isPendingProductionBackend
    }

    /// Returns true only for an exact, portable production verifier label.
    public static func isProductionVerifyBackendLabel(_ raw: String?) -> Bool {
        guard let raw else {
            return false
        }
        let backend = raw
        if backend.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            || backend.trimmingCharacters(in: .whitespacesAndNewlines) != backend
            || !isPortableVerifierBackendLabel(backend)
            || isPendingProductionBackendLabel(backend)
            || isProductionClaimBackendLabel(backend)
            || isTrustedSetupBackendLabel(backend)
            || isDeveloperOnlyBackendLabel(backend)
        {
            return false
        }
        return backend == "halo2/ipa"
            || starkFriProductionBackends.contains(backend)
            || productionNativeHalo2PastaBackends.contains(backend)
    }

    /// Requires an exact production verifier label and returns it unchanged.
    @discardableResult
    public static func requireProductionVerifyBackendLabel(
        _ raw: String,
        context: String = "backend"
    ) throws -> String {
        let backend = raw
        guard !backend.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw VerifyingKeyBackendTagValidationError.blankBackend(context: context)
        }
        guard backend.trimmingCharacters(in: .whitespacesAndNewlines) == backend else {
            throw VerifyingKeyBackendTagValidationError.surroundingWhitespace(context: context)
        }
        guard isProductionVerifyBackendLabel(backend) else {
            throw VerifyingKeyBackendTagValidationError.unsupportedProductionBackend(
                context: context,
                backend: backend
            )
        }
        return backend
    }

    private static let starkFriProductionBackends: Set<String> = [
        "stark/fri",
        "stark/fri/sha256-goldilocks",
        "stark/fri/poseidon2-goldilocks",
        "stark/fri/sha256_goldilocks.v1"
    ]

    private static let productionNativeHalo2PastaBackends: Set<String> = [
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
        "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
        "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
        "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4"
    ]

    private static let trustedSetupBackendSegments: Set<String> = [
        "groth16", "kzg", "bn254", "bn256", "bls12", "srs", "crs",
        "ptau", "ceremony", "powersoftau"
    ]

    private static let trustedSetupCompactTokens = [
        "groth16", "kzg", "bn254", "bn256", "bls12381", "bls12",
        "srs", "crs", "ptau", "ceremony", "trustedsetup",
        "structuredreferencestring", "universalsrs", "powersoftau"
    ]

    private static let productionClaimBackendFragments = [
        "productionready", "productionhardened", "productionenabled",
        "productionapproved", "productioncertified", "productionclaim",
        "claimedproduction", "mainnetready", "mainnetcomplete", "mainnetclaim",
        "claimedmainnet", "mainnetcertified", "mainnetapproved", "mainnetrelease",
        "auditedproduction", "externallyaudited", "thirdpartyaudited",
        "boiaudited", "auditedmainnet", "externalaudit", "auditpassed",
        "auditapproved", "auditsignoff", "auditclaim", "claimedaudit",
        "securityreviewpassed", "securityauditpassed", "securityaudited",
        "externalsecurityreview", "certifiedproduction", "certifiedmainnet",
        "releaseready", "releaseapproved", "releasecertified"
    ]

    private static func isProductionClaimBackendLabel(_ raw: String) -> Bool {
        let compact = compactAscii(raw.lowercased())
        return productionClaimBackendFragments.contains(where: { compact.contains($0) })
    }

    private static func isTrustedSetupBackendLabel(_ raw: String) -> Bool {
        let label = raw.lowercased()
        let compact = compactAscii(label)
        return lowercaseAsciiSegments(label).contains(where: {
            trustedSetupBackendSegments.contains($0)
        }) || trustedSetupCompactTokens.contains(where: { compact.contains($0) })
    }

    private static func isDeveloperOnlyBackendLabel(_ raw: String) -> Bool {
        let label = raw.lowercased()
        let compact = compactAscii(label)
        if [
            "notforproduction", "notproduction", "notproductionready", "notready",
            "replacebeforeproduction", "replacebeforemainnet", "draftonly"
        ].contains(where: { compact.contains($0) }) {
            return true
        }

        var letterRun = ""
        for token in lowercaseAsciiSegments(label) {
            if isDeveloperOnlyBackendRun(token) {
                return true
            }
            if token.count == 1 {
                letterRun += token
            } else {
                if isDeveloperOnlyBackendRun(letterRun) {
                    return true
                }
                letterRun = ""
            }
        }
        return isDeveloperOnlyBackendRun(letterRun)
    }

    private static func isDeveloperOnlyBackendRun(_ value: String) -> Bool {
        value.contains("debug")
            || value.contains("mock")
            || value.contains("fixture")
            || value.contains("dev")
            || value.contains("todo")
            || value.contains("draft")
            || value.contains("pending")
            || value.contains("replace")
            || value == "test"
            || value == "dummy"
            || value == "fake"
            || value == "stub"
            || value == "sample"
            || value == "placeholder"
    }

    private static func isPortableVerifierBackendLabel(_ value: String) -> Bool {
        guard let first = value.unicodeScalars.first, let last = value.unicodeScalars.last else {
            return false
        }
        let isLowerAsciiAlphanumeric: (Unicode.Scalar) -> Bool = { scalar in
            (48 ... 57).contains(scalar.value) || (97 ... 122).contains(scalar.value)
        }
        guard isLowerAsciiAlphanumeric(first), isLowerAsciiAlphanumeric(last) else {
            return false
        }
        guard value.unicodeScalars.allSatisfy({ scalar in
            isLowerAsciiAlphanumeric(scalar)
                || scalar == "/"
                || scalar == ":"
                || scalar == "."
                || scalar == "_"
                || scalar == "-"
        }) else {
            return false
        }
        return !["//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:"].contains {
            value.contains($0)
        }
    }

    private static func lowercaseAsciiSegments(_ value: String) -> [String] {
        value.split(whereSeparator: { !$0.isASCII || !$0.isLetter && !$0.isNumber })
            .map(String.init)
    }

    private static func compactAscii(_ value: String) -> String {
        String(value.filter { $0.isASCII && ($0.isLetter || $0.isNumber) })
    }
}

/// Classification of a human-facing verifier catalog label. This type is
/// deliberately separate from the two-case Norito engine enum.
public enum VerifierBackendCatalogTag: Sendable, Equatable {
    case production
    case pending
    case unsupported

    /// Classify an exact known catalog label without enabling pending entries.
    public init(catalogLabel raw: String) {
        let label = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !label.isEmpty,
              !label.unicodeScalars.contains(where: { $0.value > 127 })
        else {
            self = .unsupported
            return
        }
        let compact = String(label.filter { $0.isASCII && ($0.isLetter || $0.isNumber) })
        if Self.pendingCompactLabels.contains(compact) {
            self = .pending
        } else if VerifyingKeyBackendTag(canonicalLabel: label) != nil
            || Self.productionLabels.contains(label)
        {
            self = .production
        } else {
            self = .unsupported
        }
    }

    /// Returns true only for a known catalog entry that is still fail-closed.
    public var isPendingProductionBackend: Bool {
        self == .pending
    }

    private static let pendingCompactLabels: Set<String> = [
        "halo2ipaorchard", "orchard", "zcashorchard",
        "groth16bls12377", "groth16bls12377decaf377", "bls12377",
        "decaf377", "masp", "penumbra", "penumbramasp", "halo2ipapenumbra",
        "halo2ipamasp", "fcmppluspluscurvetree", "fcmp", "monero",
        "monerofcmp", "monerofcmpplusplus", "curvetree", "halo2ipamonero",
        "halo2ipacurvetree", "latticepcssis", "latticepcszk", "jindo",
        "jindolatticepcszk", "jindolatticepcszkv0", "jindolatticepcssis",
        "starkfrimiden", "midenstark", "aztecplonkishprivatekernel",
        "aztecprivatekernel", "pqmaspstarkfri", "pqmaspstark",
        "starkfripqmaspstarkfri", "postquantummasp", "anonymouspgc",
        "anonymouspgckoutofn", "anonymouspgckoutofnv1", "verange",
        "verangetransparentrange", "verangetransparentrangev1", "zkat",
        "zkatpolicyprivateauthenticator", "zkatpolicyprivateauthv1",
        "recursiveanonymousadmission", "recursiveanonymousadmissionv0",
        "zkamsrecursiveadmission", "zkamsrecursiveadmissionv0",
        "vegaexistingcredentialzk", "vegaexistingcredentialzkv0",
        "silentthresholdanoncred", "silentthresholdanoncredv0",
        "silentthresholdanonymouscredential", "thresholdanonymouscredentials",
        "zkx509", "zkvmx509identity", "zkx509onchainidentity",
        "zkx509onchainidentityv0", "siswithhints", "sishints",
        "sishintsanoncredpqv0", "latticeanonymouscredentials"
    ]

    private static let productionLabels: Set<String> = [
        "halo2/ipa",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
        "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
        "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
        "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
        "stark/fri",
        "stark/fri/sha256-goldilocks",
        "stark/fri/poseidon2-goldilocks",
        "stark/fri/sha256_goldilocks.v1"
    ]
}

public enum VerifyingKeyBackendTagValidationError: Error, Equatable, LocalizedError {
    case blankBackend(context: String)
    case surroundingWhitespace(context: String)
    case unsupportedProductionBackend(context: String, backend: String)

    public var errorDescription: String? {
        switch self {
        case let .blankBackend(context):
            return "\(context) must not be blank."
        case let .surroundingWhitespace(context):
            return "\(context) must not contain surrounding whitespace."
        case let .unsupportedProductionBackend(context, backend):
            return "\(context) uses unsupported production verifier backend \(backend)."
        }
    }
}

public enum VerifierBackendRegistryLabelValidationError: Error, Equatable, LocalizedError {
    case unsupported(context: String, label: String)

    public var errorDescription: String? {
        switch self {
        case let .unsupported(context, label):
            return "\(context) is not an exact supported verifier-registry label: \(label)."
        }
    }
}

/// Exact verifier-registry identifiers accepted by native Rust dispatch.
public enum VerifierBackendRegistryLabels {
    public static func isSupported(_ label: String?) -> Bool {
        VerifyingKeyBackendTag.isProductionVerifyBackendLabel(label)
    }

    @discardableResult
    public static func requireSupported(
        _ label: String,
        context: String = "backend"
    ) throws -> String {
        guard isSupported(label) else {
            throw VerifierBackendRegistryLabelValidationError.unsupported(
                context: context,
                label: label
            )
        }
        return label
    }
}
