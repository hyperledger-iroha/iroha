package org.hyperledger.iroha.sdk.core.model.zk

import java.util.Collections

/**
 * Low-level proof engines encoded by `iroha_data_model::zk::BackendTag`.
 *
 * Protocol profiles are deliberately not enum variants. They are selected by
 * one exact label from [VERIFIER_BACKEND_REGISTRY_LABELS_V1].
 */
enum class VerifyingKeyBackendTag(@JvmField val noritoValue: String) {
    HALO2_IPA_PASTA("halo2-ipa-pasta"),
    STARK("stark");

    companion object {
        /**
         * Exact native verifier configurations admitted by registry v1.
         *
         * Equality is byte-for-byte. Callers must not trim, case-fold, infer a
         * family, or accept aliases.
         */
        @JvmField
        val VERIFIER_BACKEND_REGISTRY_LABELS_V1: Set<String> = Collections.unmodifiableSet(
            linkedSetOf(
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
                "stark/fri/sha256_goldilocks.v1",
            ),
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
            "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4" ->
                HALO2_IPA_PASTA
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1" ->
                STARK
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
    }
}
