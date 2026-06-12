import Foundation

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

public enum VerifyingKeyBackendTag: UInt32, CaseIterable, Sendable, Equatable {
    case halo2IpaPasta = 0
    case halo2Bn254 = 1
    case groth16 = 2
    case stark = 3
    case unsupported = 4
    case halo2IpaOrchard = 5
    case groth16Bls12377 = 6
    case fcmpPlusPlusCurveTree = 7
    case latticePcsSis = 8
    case midenStark = 9
    case aztecPlonkishPrivateKernel = 10
    case pqMaspStarkFri = 11
    case anonymousPgc = 12
    case veRange = 13
    case zkAt = 14
    case recursiveAnonymousAdmission = 15
    case vegaExistingCredentialZk = 16
    case silentThresholdAnoncred = 17
    case zkX509 = 18
    case sisWithHints = 19

    public var noritoDiscriminant: UInt32 { rawValue }

    public var canonicalLabel: String {
        switch self {
        case .halo2IpaPasta:
            return "halo2-ipa-pasta"
        case .halo2Bn254:
            return "halo2-bn254"
        case .groth16:
            return "groth16"
        case .stark:
            return "stark"
        case .unsupported:
            return "unsupported"
        case .halo2IpaOrchard:
            return "halo2-ipa-orchard"
        case .groth16Bls12377:
            return "groth16-bls12-377"
        case .fcmpPlusPlusCurveTree:
            return "fcmp-plus-plus-curve-tree"
        case .latticePcsSis:
            return "lattice-pcs-sis"
        case .midenStark:
            return "miden-stark"
        case .aztecPlonkishPrivateKernel:
            return "aztec-plonkish-private-kernel"
        case .pqMaspStarkFri:
            return "pq-masp-stark-fri"
        case .anonymousPgc:
            return "anonymous-pgc"
        case .veRange:
            return "verange"
        case .zkAt:
            return "zkat"
        case .recursiveAnonymousAdmission:
            return "recursive-anonymous-admission"
        case .vegaExistingCredentialZk:
            return "vega-existing-credential-zk"
        case .silentThresholdAnoncred:
            return "silent-threshold-anoncred"
        case .zkX509:
            return "zk-x509"
        case .sisWithHints:
            return "sis-with-hints"
        }
    }

    public var isPendingProductionBackend: Bool {
        switch self {
        case .halo2IpaOrchard,
             .groth16Bls12377,
             .fcmpPlusPlusCurveTree,
             .latticePcsSis,
             .midenStark,
             .aztecPlonkishPrivateKernel,
             .pqMaspStarkFri,
             .anonymousPgc,
             .veRange,
             .zkAt,
             .recursiveAnonymousAdmission,
             .vegaExistingCredentialZk,
             .silentThresholdAnoncred,
             .zkX509,
             .sisWithHints:
            return true
        case .halo2IpaPasta,
             .halo2Bn254,
             .groth16,
             .stark,
             .unsupported:
            return false
        }
    }

    public init(catalogLabel raw: String) {
        let label = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if label.isEmpty {
            self = .unsupported
            return
        }
        if label.unicodeScalars.contains(where: { $0.value > 127 }) {
            self = .unsupported
            return
        }
        let compact = Self.compactAscii(label)
        self = Self.catalogBackendAliases[compact] ?? .unsupported
    }

    public static func isPendingProductionBackendLabel(_ raw: String) -> Bool {
        VerifyingKeyBackendTag(catalogLabel: raw).isPendingProductionBackend
    }

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
            || isDeveloperOnlyBackendLabel(backend) {
            return false
        }
        return backend == "halo2/ipa"
            || isStarkFriProductionBackendLabel(backend)
            || isNativeHalo2PastaProductionBackendLabel(backend)
    }

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

    private static let productionNativeHalo2PastaBackends: Set<String> = [
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/offline-note-recursive",
        "halo2/pasta/kagemusha-folded-v1",
        "halo2/pasta/kagemusha-recursive-aggregation-v1",
        "halo2/pasta/kagemusha-recursive-compact-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-append-v1",
        "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
        "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
        "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified"
    ]

    private static let catalogBackendAliases: [String: VerifyingKeyBackendTag] = [
        "unsupported": .unsupported,
        "halo2ipa": .halo2IpaPasta,
        "halo2ipapasta": .halo2IpaPasta,
        "halo2pasta": .halo2IpaPasta,
        "halo2pastaipavotebool": .halo2IpaPasta,
        "halo2bn254": .halo2Bn254,
        "groth16": .groth16,
        "groth16bn254": .groth16,
        "stark": .stark,
        "starkfri": .stark,
        "starkfrisha256goldilocks": .stark,
        "starkfriposeidon2goldilocks": .stark,
        "starkfrisha256goldilocksv1": .stark,
        "halo2ipaorchard": .halo2IpaOrchard,
        "orchard": .halo2IpaOrchard,
        "zcashorchard": .halo2IpaOrchard,
        "groth16bls12377": .groth16Bls12377,
        "groth16bls12377decaf377": .groth16Bls12377,
        "bls12377": .groth16Bls12377,
        "decaf377": .groth16Bls12377,
        "masp": .groth16Bls12377,
        "penumbra": .groth16Bls12377,
        "penumbramasp": .groth16Bls12377,
        "halo2ipapenumbra": .groth16Bls12377,
        "halo2ipamasp": .groth16Bls12377,
        "fcmppluspluscurvetree": .fcmpPlusPlusCurveTree,
        "fcmp": .fcmpPlusPlusCurveTree,
        "monero": .fcmpPlusPlusCurveTree,
        "monerofcmp": .fcmpPlusPlusCurveTree,
        "monerofcmpplusplus": .fcmpPlusPlusCurveTree,
        "curvetree": .fcmpPlusPlusCurveTree,
        "halo2ipamonero": .fcmpPlusPlusCurveTree,
        "halo2ipacurvetree": .fcmpPlusPlusCurveTree,
        "latticepcssis": .latticePcsSis,
        "latticepcszk": .latticePcsSis,
        "jindo": .latticePcsSis,
        "jindolatticepcszk": .latticePcsSis,
        "jindolatticepcszkv0": .latticePcsSis,
        "jindolatticepcssis": .latticePcsSis,
        "starkfrimiden": .midenStark,
        "midenstark": .midenStark,
        "aztecplonkishprivatekernel": .aztecPlonkishPrivateKernel,
        "aztecprivatekernel": .aztecPlonkishPrivateKernel,
        "pqmaspstarkfri": .pqMaspStarkFri,
        "pqmaspstark": .pqMaspStarkFri,
        "starkfripqmaspstarkfri": .pqMaspStarkFri,
        "postquantummasp": .pqMaspStarkFri,
        "anonymouspgc": .anonymousPgc,
        "anonymouspgckoutofn": .anonymousPgc,
        "anonymouspgckoutofnv1": .anonymousPgc,
        "verange": .veRange,
        "verangetransparentrange": .veRange,
        "verangetransparentrangev1": .veRange,
        "zkat": .zkAt,
        "zkatpolicyprivateauthenticator": .zkAt,
        "zkatpolicyprivateauthv1": .zkAt,
        "recursiveanonymousadmission": .recursiveAnonymousAdmission,
        "recursiveanonymousadmissionv0": .recursiveAnonymousAdmission,
        "zkamsrecursiveadmission": .recursiveAnonymousAdmission,
        "zkamsrecursiveadmissionv0": .recursiveAnonymousAdmission,
        "vegaexistingcredentialzk": .vegaExistingCredentialZk,
        "vegaexistingcredentialzkv0": .vegaExistingCredentialZk,
        "silentthresholdanoncred": .silentThresholdAnoncred,
        "silentthresholdanoncredv0": .silentThresholdAnoncred,
        "silentthresholdanonymouscredential": .silentThresholdAnoncred,
        "thresholdanonymouscredentials": .silentThresholdAnoncred,
        "zkx509": .zkX509,
        "zkvmx509identity": .zkX509,
        "zkx509onchainidentity": .zkX509,
        "zkx509onchainidentityv0": .zkX509,
        "siswithhints": .sisWithHints,
        "sishints": .sisWithHints,
        "sishintsanoncredpqv0": .sisWithHints,
        "latticeanonymouscredentials": .sisWithHints
    ]

    private static let starkFriProductionBackends: Set<String> = [
        "stark/fri",
        "stark/fri/sha256-goldilocks",
        "stark/fri/poseidon2-goldilocks",
        "stark/fri/sha256_goldilocks.v1"
    ]

    private static let trustedSetupBackendSegments: Set<String> = [
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "powersoftau"
    ]

    private static let trustedSetupCompactTokens = [
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12381",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "trustedsetup",
        "structuredreferencestring",
        "universalsrs",
        "powersoftau"
    ]

    private static let productionClaimBackendFragments = [
        "productionready",
        "productionhardened",
        "productionenabled",
        "productionapproved",
        "productioncertified",
        "productionclaim",
        "claimedproduction",
        "mainnetready",
        "mainnetcomplete",
        "mainnetclaim",
        "claimedmainnet",
        "mainnetcertified",
        "mainnetapproved",
        "mainnetrelease",
        "auditedproduction",
        "externallyaudited",
        "thirdpartyaudited",
        "boiaudited",
        "auditedmainnet",
        "externalaudit",
        "auditpassed",
        "auditapproved",
        "auditsignoff",
        "auditclaim",
        "claimedaudit",
        "securityreviewpassed",
        "securityauditpassed",
        "securityaudited",
        "externalsecurityreview",
        "certifiedproduction",
        "certifiedmainnet",
        "releaseready",
        "releaseapproved",
        "releasecertified"
    ]

    private static func isTrustedSetupBackendLabel(_ raw: String) -> Bool {
        let label = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let compact = compactAscii(label)
        if lowercaseAsciiSegments(label).contains(where: { trustedSetupBackendSegments.contains($0) }) {
            return true
        }
        if trustedSetupCompactTokens.contains(where: { compact.contains($0) }) {
            return true
        }
        return label == "groth16"
            || label.hasPrefix("groth16/")
            || label == "kzg"
            || label.hasPrefix("kzg/")
            || label == "bn254"
            || label == "bn256"
            || label == "bls12_381"
            || label == "bls12-381"
            || label == "halo2/bn254"
            || label.hasPrefix("halo2/bn254/")
            || label.contains("/bn254")
            || label.contains(":bn254")
            || label.contains("/bn256")
            || label.contains(":bn256")
            || label.contains("/bls12")
            || label.contains(":bls12")
            || label == "halo2/kzg"
            || label.hasPrefix("halo2/kzg/")
            || label.contains("/kzg")
            || label.contains(":kzg")
    }

    private static func isDeveloperOnlyBackendLabel(_ raw: String) -> Bool {
        let label = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        var letterRun = ""
        for token in lowercaseAsciiSegments(label) {
            if isDeveloperOnlyBackendRun(token) {
                return true
            }
            if token.count == 1 {
                letterRun += token
                continue
            }
            if isDeveloperOnlyBackendRun(letterRun) {
                return true
            }
            letterRun = ""
        }
        return isDeveloperOnlyBackendRun(letterRun)
    }

    private static func isProductionClaimBackendLabel(_ raw: String) -> Bool {
        let compact = compactAscii(raw.lowercased())
        return productionClaimBackendFragments.contains(where: { compact.contains($0) })
    }

    private static func isDeveloperOnlyBackendRun(_ value: String) -> Bool {
        value.contains("debug")
            || value.contains("mock")
            || value.contains("fixture")
            || value.contains("dev")
            || value == "test"
            || value == "dummy"
            || value == "fake"
            || value == "stub"
            || value == "sample"
            || value == "placeholder"
    }

    private static func isStarkFriProductionBackendLabel(_ backend: String) -> Bool {
        starkFriProductionBackends.contains(backend)
    }

    private static func isNativeHalo2PastaProductionBackendLabel(_ backend: String) -> Bool {
        guard let normalized = normalizeNativeHalo2PastaBackendLabel(backend) else {
            return false
        }
        return productionNativeHalo2PastaBackends.contains(normalized)
    }

    private static func normalizeNativeHalo2PastaBackendLabel(_ raw: String) -> String? {
        let backend = raw
        if backend.isEmpty || backend.trimmingCharacters(in: .whitespacesAndNewlines) != backend {
            return nil
        }
        for (prefix, targetPrefix) in [
            ("halo2/pasta/ipa/", "halo2/pasta/"),
            ("halo2/pasta/", "halo2/pasta/"),
            ("halo2/ipa::", "halo2/pasta/"),
            ("halo2/ipa:", "halo2/pasta/"),
            ("halo2/ipa/", "halo2/pasta/")
        ] {
            if backend.hasPrefix(prefix) {
                let rest = String(backend.dropFirst(prefix.count))
                return rest.isEmpty ? nil : "\(targetPrefix)\(rest)"
            }
        }
        return nil
    }

    private static func isPortableVerifierBackendLabel(_ value: String) -> Bool {
        guard let first = value.unicodeScalars.first, let last = value.unicodeScalars.last else {
            return false
        }
        let isLowerAsciiAlphanumeric: (Unicode.Scalar) -> Bool = { scalar in
            let codepoint = scalar.value
            return (48...57).contains(codepoint) || (97...122).contains(codepoint)
        }
        guard isLowerAsciiAlphanumeric(first), isLowerAsciiAlphanumeric(last) else {
            return false
        }
        guard value.unicodeScalars.allSatisfy({ scalar in
            let codepoint = scalar.value
            return (48...57).contains(codepoint)
                || (97...122).contains(codepoint)
                || scalar == "/"
                || scalar == ":"
                || scalar == "."
                || scalar == "_"
                || scalar == "-"
        }) else {
            return false
        }
        return !["//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:"].contains { separator in
            value.contains(separator)
        }
    }

    private static func lowercaseAsciiSegments(_ value: String) -> [String] {
        var segments: [String] = []
        var current = ""
        for scalar in value.unicodeScalars {
            let codepoint = scalar.value
            if (48...57).contains(codepoint) || (97...122).contains(codepoint) {
                current.unicodeScalars.append(scalar)
            } else if !current.isEmpty {
                segments.append(current)
                current = ""
            }
        }
        if !current.isEmpty {
            segments.append(current)
        }
        return segments
    }

    private static func compactAscii(_ value: String) -> String {
        var compact = ""
        for scalar in value.unicodeScalars {
            let codepoint = scalar.value
            if (48...57).contains(codepoint) || (97...122).contains(codepoint) {
                compact.unicodeScalars.append(scalar)
            }
        }
        return compact
    }
}
