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
            val normalized = value.trim().lowercase()
            for (tag in entries) {
                if (tag.noritoValue == normalized) {
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

            return when {
                label == "unsupported" || compact == "unsupported" -> UNSUPPORTED
                compact.contains("pqmasp") || compact.contains("postquantummasp") ->
                    PQ_MASP_STARK_FRI
                compact.contains("anonymouspgc") || compact.contains("pgckoutofn") ->
                    ANONYMOUS_PGC
                compact.contains("verange") -> VERANGE
                compact.contains("zkat") || compact.contains("policyprivateauthenticator") ->
                    ZKAT
                compact.contains("zkams") || compact.contains("recursiveanonymousadmission") ->
                    RECURSIVE_ANONYMOUS_ADMISSION
                compact.contains("vega") || compact.contains("existingcredentialzk") ->
                    VEGA_EXISTING_CREDENTIAL_ZK
                compact.contains("silentthreshold") ||
                    compact.contains("thresholdanonymouscredential") ->
                    SILENT_THRESHOLD_ANONCRED
                compact.contains("zkx509") || compact.contains("x509") ||
                    compact.contains("zkvmx509") ->
                    ZK_X509
                compact.contains("siswithhints") || compact.contains("sishints") ||
                    compact.contains("latticeanonymouscredentials") ->
                    SIS_WITH_HINTS
                compact.contains("orchard") || compact.contains("zcashorchard") ->
                    HALO2_IPA_ORCHARD
                compact.contains("penumbra") || compact.contains("masp") ||
                    compact.contains("bls12377") || compact.contains("decaf377") ->
                    GROTH16_BLS12_377
                compact.contains("fcmp") || compact.contains("monero") ||
                    compact.contains("curvetree") ->
                    FCMP_PLUS_PLUS_CURVE_TREE
                compact.contains("lattice") || compact.contains("pcssis") ||
                    compact.contains("jindo") ->
                    LATTICE_PCS_SIS
                compact.contains("miden") -> MIDEN_STARK
                compact.contains("aztec") -> AZTEC_PLONKISH_PRIVATE_KERNEL
                compact.contains("halo2") && compact.contains("bn254") -> HALO2_BN254
                compact.contains("groth16") -> GROTH16
                compact.contains("stark") -> STARK
                compact == "halo2ipa" ||
                    compact == "halo2ipapasta" ||
                    compact == "halo2pasta" ||
                    (compact.contains("halo2") &&
                        (compact.contains("ipa") || compact.contains("pasta"))) ->
                    HALO2_IPA_PASTA
                else -> UNSUPPORTED
            }
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
            "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
            "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
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
            "auditedproduction",
            "externallyaudited",
            "auditpassed",
            "auditapproved",
            "auditsignoff",
            "auditclaim",
            "claimedaudit",
            "securityreviewpassed",
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
            val embedded = listOf("debug", "mock", "fixture", "dev")
            val exact = setOf("test", "dummy", "fake", "stub", "sample", "placeholder")
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

        private fun isPortableVerifierBackendLabel(value: String): Boolean =
            value.all { ch ->
                ch in '0'..'9' ||
                    ch in 'A'..'Z' ||
                    ch in 'a'..'z' ||
                    ch == '/' ||
                    ch == ':' ||
                    ch == '.' ||
                    ch == '_' ||
                    ch == '-'
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
