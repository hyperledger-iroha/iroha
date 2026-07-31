package org.hyperledger.iroha.sdk.core.model.zk

import java.util.Collections

/**
 * Low-level proof engines encoded by `iroha_data_model::zk::BackendTag`.
 *
 * Privacy protocols and verifier profiles remain separate catalog labels and
 * never become Norito enum variants.
 */
enum class VerifyingKeyBackendTag(@JvmField val noritoValue: String) {
    HALO2_IPA_PASTA("halo2-ipa-pasta"),
    STARK("stark");

    companion object {
        /** Exact native verifier configurations admitted by registry v1. */
        @JvmField
        val VERIFIER_BACKEND_REGISTRY_LABELS_V1: Set<String> = Collections.unmodifiableSet(
            linkedSetOf(
                "halo2/ipa",
                "halo2/pasta/kaigi-roster-v1",
                "halo2/pasta/kaigi-usage-v1",
                "halo2/pasta/ivm-execution-v1",
                "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
                "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
                "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
                "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
                "stark/fri",
                "stark/fri/sha256-goldilocks",
                "stark/fri/poseidon2-goldilocks",
                "stark/fri/sha256_goldilocks.v1",
            ),
        )

        private val starkFriProductionBackends = setOf(
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        )

        private val productionNativeHalo2PastaBackends =
            VERIFIER_BACKEND_REGISTRY_LABELS_V1.filterTo(linkedSetOf()) {
                it.startsWith("halo2/pasta/")
            }

        private val pendingCatalogBackendAliases = setOf(
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
            "sishintsanoncredpqv0", "latticeanonymouscredentials",
        )

        private val productionClaimBackendFragments = listOf(
            "productionready", "productionhardened", "productionenabled",
            "productionapproved", "productioncertified", "productionclaim",
            "claimedproduction", "mainnetready", "mainnetcomplete", "mainnetclaim",
            "claimedmainnet", "mainnetcertified", "mainnetapproved", "mainnetrelease",
            "auditedproduction", "externallyaudited", "thirdpartyaudited",
            "boiaudited", "auditedmainnet", "externalaudit", "auditpassed",
            "auditapproved", "auditsignoff", "auditclaim", "claimedaudit",
            "securityreviewpassed", "securityauditpassed", "securityaudited",
            "externalsecurityreview", "certifiedproduction", "certifiedmainnet",
            "releaseready", "releaseapproved", "releasecertified",
        )

        private val trustedSetupBackendSegments = setOf(
            "groth16", "kzg", "bn254", "bn256", "bls12", "srs", "crs",
            "ptau", "ceremony", "powersoftau",
        )

        private val trustedSetupCompactTokens = setOf(
            "groth16", "kzg", "bn254", "bn256", "bls12381", "bls12",
            "srs", "crs", "ptau", "ceremony", "trustedsetup",
            "structuredreferencestring", "universalsrs", "powersoftau",
        )

        /** Parses one exact canonical Norito engine label. */
        @JvmStatic
        fun parse(value: String): VerifyingKeyBackendTag = when (value) {
            HALO2_IPA_PASTA.noritoValue -> HALO2_IPA_PASTA
            STARK.noritoValue -> STARK
            else -> throw IllegalArgumentException("unsupported backend tag: $value")
        }

        /** Resolves one exact registry label to its low-level proof engine. */
        @JvmStatic
        fun verifierBackendRegistryTagV1(label: String?): VerifyingKeyBackendTag? = when (label) {
            in productionNativeHalo2PastaBackends, "halo2/ipa" -> HALO2_IPA_PASTA
            in starkFriProductionBackends -> STARK
            else -> null
        }

        /** Returns true only for an exact registry-v1 label. */
        @JvmStatic
        fun isVerifierBackendRegistryLabelV1(raw: String?): Boolean =
            verifierBackendRegistryTagV1(raw) != null

        /** Requires an exact registry-v1 label and returns it unchanged. */
        @JvmStatic
        @JvmOverloads
        fun requireVerifierBackendRegistryLabelV1(
            raw: String,
            context: String = "backend",
        ): String {
            require(isVerifierBackendRegistryLabelV1(raw)) {
                "$context uses unsupported verifier-registry label $raw"
            }
            return raw
        }

        /** Classifies a known catalog alias without enabling pending entries. */
        @JvmStatic
        fun fromCatalogLabel(raw: String?): VerifyingKeyBackendCatalogTag {
            val label = raw?.trim()?.lowercase() ?: return VerifyingKeyBackendCatalogTag.UNSUPPORTED
            if (label.isEmpty() || label.any { it.code > 0x7F }) {
                return VerifyingKeyBackendCatalogTag.UNSUPPORTED
            }
            val compact = compactAscii(label)
            return when {
                pendingCatalogBackendAliases.contains(compact) ->
                    VerifyingKeyBackendCatalogTag.PENDING
                parseOrNull(label) != null || VERIFIER_BACKEND_REGISTRY_LABELS_V1.contains(label) ->
                    VerifyingKeyBackendCatalogTag.PRODUCTION
                else -> VerifyingKeyBackendCatalogTag.UNSUPPORTED
            }
        }

        /** Returns true only for a known, pending production catalog label. */
        @JvmStatic
        fun isPendingProductionBackendLabel(raw: String?): Boolean =
            fromCatalogLabel(raw).isPendingProductionBackend

        /** Returns true only for an exact, portable production verifier label. */
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
                starkFriProductionBackends.contains(backend) ||
                productionNativeHalo2PastaBackends.contains(backend)
        }

        /** Requires an exact production verifier label and returns it unchanged. */
        @JvmStatic
        @JvmOverloads
        fun requireProductionVerifyBackendLabel(
            raw: String,
            context: String = "backend",
        ): String {
            require(raw.isNotBlank()) { "$context must not be blank" }
            require(raw.trim() == raw) { "$context must not contain surrounding whitespace" }
            require(isProductionVerifyBackendLabel(raw)) {
                "$context uses unsupported production verifier backend $raw"
            }
            return raw
        }

        private fun parseOrNull(value: String): VerifyingKeyBackendTag? = when (value) {
            HALO2_IPA_PASTA.noritoValue -> HALO2_IPA_PASTA
            STARK.noritoValue -> STARK
            else -> null
        }

        private fun isProductionClaimBackendLabel(raw: String): Boolean {
            val compact = compactAscii(raw.lowercase())
            return productionClaimBackendFragments.any { compact.contains(it) }
        }

        private fun isTrustedSetupBackendLabel(raw: String): Boolean {
            val label = raw.lowercase()
            val compact = compactAscii(label)
            return lowercaseAsciiSegments(label).any { trustedSetupBackendSegments.contains(it) } ||
                trustedSetupCompactTokens.any { compact.contains(it) }
        }

        private fun isDeveloperOnlyBackendLabel(raw: String): Boolean {
            val label = raw.lowercase()
            val compact = compactAscii(label)
            if (listOf(
                    "notforproduction", "notproduction", "notproductionready", "notready",
                    "replacebeforeproduction", "replacebeforemainnet", "draftonly",
                ).any { compact.contains(it) }
            ) {
                return true
            }
            val letterRun = StringBuilder()
            for (token in lowercaseAsciiSegments(label)) {
                if (isDeveloperOnlyBackendRun(token)) {
                    return true
                }
                if (token.length == 1) {
                    letterRun.append(token)
                } else {
                    if (isDeveloperOnlyBackendRun(letterRun.toString())) {
                        return true
                    }
                    letterRun.setLength(0)
                }
            }
            return isDeveloperOnlyBackendRun(letterRun.toString())
        }

        private fun isDeveloperOnlyBackendRun(value: String): Boolean =
            value.contains("debug") ||
                value.contains("mock") ||
                value.contains("fixture") ||
                value.contains("dev") ||
                value.contains("todo") ||
                value.contains("draft") ||
                value.contains("pending") ||
                value.contains("replace") ||
                value in setOf("test", "dummy", "fake", "stub", "sample", "placeholder")

        private fun isPortableVerifierBackendLabel(value: String): Boolean {
            if (value.isEmpty()) return false
            fun lowerAsciiAlphanumeric(ch: Char): Boolean = ch in '0'..'9' || ch in 'a'..'z'
            if (!lowerAsciiAlphanumeric(value.first()) || !lowerAsciiAlphanumeric(value.last())) {
                return false
            }
            if (!value.all {
                    lowerAsciiAlphanumeric(it) ||
                        it == '/' || it == ':' || it == '.' || it == '_' || it == '-'
                }
            ) {
                return false
            }
            return listOf("//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:")
                .none { value.contains(it) }
        }

        private fun lowercaseAsciiSegments(value: String): List<String> =
            value.split(Regex("[^a-z0-9]+")).filter { it.isNotEmpty() }

        private fun compactAscii(value: String): String =
            value.filter { it in 'a'..'z' || it in '0'..'9' }
    }
}

/** Human-facing verifier catalog classification separate from the wire enum. */
enum class VerifyingKeyBackendCatalogTag(val isPendingProductionBackend: Boolean) {
    PRODUCTION(false),
    PENDING(true),
    UNSUPPORTED(false),
}
