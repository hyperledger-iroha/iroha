package org.hyperledger.iroha.sdk.core.model.zk

/**
 * Backend identifiers for verifying key records.
 *
 * Matches the Norito enums exposed by `iroha_data_model::zk::BackendTag`.
 */
enum class VerifyingKeyBackendTag(@JvmField val noritoValue: String) {
    HALO2_IPA_PASTA("halo2-ipa-pasta"),
    HALO2_BN254("halo2-bn254"),
    GROTH16("groth16"),
    STARK("stark"),
    UNSUPPORTED("unsupported"),
    HALO2_IPA_ORCHARD("halo2-ipa-orchard"),
    GROTH16_BLS12_377("groth16-bls12-377"),
    FCMP_PLUS_PLUS_CURVE_TREE("fcmp-plus-plus-curve-tree"),
    LATTICE_PCS_SIS("lattice-pcs-sis"),
    MIDEN_STARK("miden-stark"),
    AZTEC_PLONKISH_PRIVATE_KERNEL("aztec-plonkish-private-kernel"),
    PQ_MASP_STARK_FRI("pq-masp-stark-fri"),
    ANONYMOUS_PGC("anonymous-pgc"),
    VERANGE("verange"),
    ZKAT("zkat"),
    RECURSIVE_ANONYMOUS_ADMISSION("recursive-anonymous-admission"),
    VEGA_EXISTING_CREDENTIAL_ZK("vega-existing-credential-zk"),
    SILENT_THRESHOLD_ANONCRED("silent-threshold-anoncred"),
    ZK_X509("zk-x509"),
    SIS_WITH_HINTS("sis-with-hints");

    val isPendingProductionBackend: Boolean
        get() = when (this) {
            HALO2_IPA_ORCHARD,
            GROTH16_BLS12_377,
            FCMP_PLUS_PLUS_CURVE_TREE,
            LATTICE_PCS_SIS,
            MIDEN_STARK,
            AZTEC_PLONKISH_PRIVATE_KERNEL,
            PQ_MASP_STARK_FRI,
            ANONYMOUS_PGC,
            VERANGE,
            ZKAT,
            RECURSIVE_ANONYMOUS_ADMISSION,
            VEGA_EXISTING_CREDENTIAL_ZK,
            SILENT_THRESHOLD_ANONCRED,
            ZK_X509,
            SIS_WITH_HINTS -> true
            HALO2_IPA_PASTA,
            HALO2_BN254,
            GROTH16,
            STARK,
            UNSUPPORTED -> false
        }

    companion object {
        /** Parses a Norito backend string into the corresponding enum value. */
        @JvmStatic
        fun parse(value: String): VerifyingKeyBackendTag {
            for (tag in entries) {
                if (tag.noritoValue == value) {
                    return tag
                }
            }
            throw IllegalArgumentException("unsupported backend tag: $value")
        }

        /** Classifies catalog and verifier labels without enabling pending production backends. */
        @JvmStatic
        fun fromCatalogLabel(raw: String?): VerifyingKeyBackendTag {
            val label = raw?.trim()?.lowercase() ?: ""
            if (label.isEmpty()) {
                return UNSUPPORTED
            }
            if (label.any { it.code > 0x7F }) {
                return UNSUPPORTED
            }
            val compact = compactAscii(label)
            return catalogBackendAliases[compact] ?: UNSUPPORTED
        }

        @JvmStatic
        fun isPendingProductionBackendLabel(raw: String?): Boolean =
            fromCatalogLabel(raw).isPendingProductionBackend

        @JvmStatic
        fun isProductionVerifyBackendLabel(raw: String?): Boolean {
            val backend = raw ?: return false
            if (backend.isBlank() ||
                backend.trim() != backend ||
                !isPortableVerifierBackendLabel(backend) ||
                isPendingProductionBackendLabel(backend) ||
                isProductionClaimBackendLabel(backend) ||
                isTrustedSetupBackendLabel(backend) ||
                isDeveloperOnlyBackendLabel(backend)
            ) {
                return false
            }
            return backend == "halo2/ipa" ||
                isStarkFriProductionBackendLabel(backend) ||
                isNativeHalo2PastaProductionBackendLabel(backend)
        }

        @JvmStatic
        fun requireProductionVerifyBackendLabel(raw: String, context: String = "backend"): String {
            val backend = raw
            require(backend.isNotBlank()) { "$context must not be blank" }
            require(backend.trim() == backend) {
                "$context must not contain surrounding whitespace"
            }
            require(isProductionVerifyBackendLabel(backend)) {
                "$context uses unsupported production verifier backend $backend"
            }
            return backend
        }

        private val productionNativeHalo2PastaBackends = setOf(
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
            "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
        )

        private val catalogBackendAliases = mapOf(
            "unsupported" to UNSUPPORTED,
            "halo2ipa" to HALO2_IPA_PASTA,
            "halo2ipapasta" to HALO2_IPA_PASTA,
            "halo2pasta" to HALO2_IPA_PASTA,
            "halo2pastaipavotebool" to HALO2_IPA_PASTA,
            "halo2bn254" to HALO2_BN254,
            "groth16" to GROTH16,
            "groth16bn254" to GROTH16,
            "stark" to STARK,
            "starkfri" to STARK,
            "starkfrisha256goldilocks" to STARK,
            "starkfriposeidon2goldilocks" to STARK,
            "starkfrisha256goldilocksv1" to STARK,
            "halo2ipaorchard" to HALO2_IPA_ORCHARD,
            "orchard" to HALO2_IPA_ORCHARD,
            "zcashorchard" to HALO2_IPA_ORCHARD,
            "groth16bls12377" to GROTH16_BLS12_377,
            "groth16bls12377decaf377" to GROTH16_BLS12_377,
            "bls12377" to GROTH16_BLS12_377,
            "decaf377" to GROTH16_BLS12_377,
            "masp" to GROTH16_BLS12_377,
            "penumbra" to GROTH16_BLS12_377,
            "penumbramasp" to GROTH16_BLS12_377,
            "halo2ipapenumbra" to GROTH16_BLS12_377,
            "halo2ipamasp" to GROTH16_BLS12_377,
            "fcmppluspluscurvetree" to FCMP_PLUS_PLUS_CURVE_TREE,
            "fcmp" to FCMP_PLUS_PLUS_CURVE_TREE,
            "monero" to FCMP_PLUS_PLUS_CURVE_TREE,
            "monerofcmp" to FCMP_PLUS_PLUS_CURVE_TREE,
            "monerofcmpplusplus" to FCMP_PLUS_PLUS_CURVE_TREE,
            "curvetree" to FCMP_PLUS_PLUS_CURVE_TREE,
            "halo2ipamonero" to FCMP_PLUS_PLUS_CURVE_TREE,
            "halo2ipacurvetree" to FCMP_PLUS_PLUS_CURVE_TREE,
            "latticepcssis" to LATTICE_PCS_SIS,
            "latticepcszk" to LATTICE_PCS_SIS,
            "jindo" to LATTICE_PCS_SIS,
            "jindolatticepcszk" to LATTICE_PCS_SIS,
            "jindolatticepcszkv0" to LATTICE_PCS_SIS,
            "jindolatticepcssis" to LATTICE_PCS_SIS,
            "starkfrimiden" to MIDEN_STARK,
            "midenstark" to MIDEN_STARK,
            "aztecplonkishprivatekernel" to AZTEC_PLONKISH_PRIVATE_KERNEL,
            "aztecprivatekernel" to AZTEC_PLONKISH_PRIVATE_KERNEL,
            "pqmaspstarkfri" to PQ_MASP_STARK_FRI,
            "pqmaspstark" to PQ_MASP_STARK_FRI,
            "starkfripqmaspstarkfri" to PQ_MASP_STARK_FRI,
            "postquantummasp" to PQ_MASP_STARK_FRI,
            "anonymouspgc" to ANONYMOUS_PGC,
            "anonymouspgckoutofn" to ANONYMOUS_PGC,
            "anonymouspgckoutofnv1" to ANONYMOUS_PGC,
            "verange" to VERANGE,
            "verangetransparentrange" to VERANGE,
            "verangetransparentrangev1" to VERANGE,
            "zkat" to ZKAT,
            "zkatpolicyprivateauthenticator" to ZKAT,
            "zkatpolicyprivateauthv1" to ZKAT,
            "recursiveanonymousadmission" to RECURSIVE_ANONYMOUS_ADMISSION,
            "recursiveanonymousadmissionv0" to RECURSIVE_ANONYMOUS_ADMISSION,
            "zkamsrecursiveadmission" to RECURSIVE_ANONYMOUS_ADMISSION,
            "zkamsrecursiveadmissionv0" to RECURSIVE_ANONYMOUS_ADMISSION,
            "vegaexistingcredentialzk" to VEGA_EXISTING_CREDENTIAL_ZK,
            "vegaexistingcredentialzkv0" to VEGA_EXISTING_CREDENTIAL_ZK,
            "silentthresholdanoncred" to SILENT_THRESHOLD_ANONCRED,
            "silentthresholdanoncredv0" to SILENT_THRESHOLD_ANONCRED,
            "silentthresholdanonymouscredential" to SILENT_THRESHOLD_ANONCRED,
            "thresholdanonymouscredentials" to SILENT_THRESHOLD_ANONCRED,
            "zkx509" to ZK_X509,
            "zkvmx509identity" to ZK_X509,
            "zkx509onchainidentity" to ZK_X509,
            "zkx509onchainidentityv0" to ZK_X509,
            "siswithhints" to SIS_WITH_HINTS,
            "sishints" to SIS_WITH_HINTS,
            "sishintsanoncredpqv0" to SIS_WITH_HINTS,
            "latticeanonymouscredentials" to SIS_WITH_HINTS,
        )

        private val trustedSetupBackendSegments = setOf(
            "groth16",
            "kzg",
            "bn254",
            "bn256",
            "bls12",
            "srs",
            "crs",
            "ptau",
            "ceremony",
            "powersoftau",
        )

        private val trustedSetupCompactTokens = setOf(
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
            "powersoftau",
        )

        private val productionClaimBackendFragments = listOf(
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
            "releasecertified",
        )

        private val starkFriProductionBackends = setOf(
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        )

        private fun isTrustedSetupBackendLabel(raw: String): Boolean {
            val label = raw.trim().lowercase()
            val compact = compactAscii(label)
            return label.split(Regex("[^a-z0-9]+")).any {
                trustedSetupBackendSegments.contains(it)
            } ||
                trustedSetupCompactTokens.any { compact.contains(it) } ||
                label == "groth16" ||
                label.startsWith("groth16/") ||
                label == "kzg" ||
                label.startsWith("kzg/") ||
                label == "bn254" ||
                label == "bn256" ||
                label == "bls12_381" ||
                label == "bls12-381" ||
                label == "halo2/bn254" ||
                label.startsWith("halo2/bn254/") ||
                label.contains("/bn254") ||
                label.contains(":bn254") ||
                label.contains("/bn256") ||
                label.contains(":bn256") ||
                label.contains("/bls12") ||
                label.contains(":bls12") ||
                label == "halo2/kzg" ||
                label.startsWith("halo2/kzg/") ||
                label.contains("/kzg") ||
                label.contains(":kzg")
        }

        private fun isDeveloperOnlyBackendLabel(raw: String): Boolean {
            val label = raw.trim().lowercase()
            val compact = compactAscii(label)
            val compactFragments =
                listOf(
                    "notforproduction",
                    "notproduction",
                    "notproductionready",
                    "notready",
                    "replacebeforeproduction",
                    "replacebeforemainnet",
                    "draftonly",
                )
            if (compactFragments.any { compact.contains(it) }) {
                return true
            }

            val embedded = listOf("debug", "mock", "fixture", "dev", "todo", "draft", "pending", "replace")
            val exact = setOf("test", "dummy", "fake", "stub", "sample", "placeholder", "todo", "draft")
            fun isDeveloperOnlyRun(run: String): Boolean =
                embedded.any { run.contains(it) } || exact.contains(run)

            val letterRun = StringBuilder()
            for (token in label.split(Regex("[^a-z0-9]+")).filter { it.isNotEmpty() }) {
                if (isDeveloperOnlyRun(token)) {
                    return true
                }
                if (token.length == 1) {
                    letterRun.append(token)
                    continue
                }
                if (isDeveloperOnlyRun(letterRun.toString())) {
                    return true
                }
                letterRun.clear()
            }
            return isDeveloperOnlyRun(letterRun.toString())
        }

        private fun isProductionClaimBackendLabel(raw: String): Boolean {
            val compact = compactAscii(raw.lowercase())
            return productionClaimBackendFragments.any { compact.contains(it) }
        }

        private fun isStarkFriProductionBackendLabel(backend: String): Boolean {
            return starkFriProductionBackends.contains(backend)
        }

        private fun isNativeHalo2PastaProductionBackendLabel(backend: String): Boolean =
            normalizeNativeHalo2PastaBackendLabel(backend)
                ?.let { productionNativeHalo2PastaBackends.contains(it) }
                ?: false

        private fun isPortableVerifierBackendLabel(value: String): Boolean {
            if (value.isEmpty()) {
                return false
            }
            fun isLowerAsciiAlphanumeric(ch: Char): Boolean = ch in '0'..'9' || ch in 'a'..'z'
            if (!isLowerAsciiAlphanumeric(value.first()) || !isLowerAsciiAlphanumeric(value.last())) {
                return false
            }
            if (!value.all { ch ->
                    isLowerAsciiAlphanumeric(ch) ||
                        ch == '/' ||
                        ch == ':' ||
                        ch == '.' ||
                        ch == '_' ||
                        ch == '-'
                }) {
                return false
            }
            return listOf("//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:").none { value.contains(it) }
        }

        private fun normalizeNativeHalo2PastaBackendLabel(raw: String): String? {
            val backend = raw
            if (backend.isEmpty() || backend.trim() != backend) {
                return null
            }
            for ((prefix, targetPrefix) in listOf(
                "halo2/pasta/ipa/" to "halo2/pasta/",
                "halo2/pasta/" to "halo2/pasta/",
                "halo2/ipa::" to "halo2/pasta/",
                "halo2/ipa:" to "halo2/pasta/",
                "halo2/ipa/" to "halo2/pasta/",
            )) {
                if (backend.startsWith(prefix)) {
                    val rest = backend.removePrefix(prefix)
                    return rest.takeIf { it.isNotEmpty() }?.let { "$targetPrefix$it" }
                }
            }
            return null
        }

        private fun compactAscii(value: String): String =
            buildString(value.length) {
                for (ch in value) {
                    if (ch in '0'..'9' || ch in 'a'..'z') {
                        append(ch)
                    }
                }
            }
    }
}
