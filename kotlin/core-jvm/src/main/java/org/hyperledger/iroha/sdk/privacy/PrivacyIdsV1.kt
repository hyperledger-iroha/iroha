// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

/** Closed first-release proof-system identity in canonical Norito discriminant order. */
enum class PrivacyProofSystemIdV1(val canonicalLabel: String) {
    STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1("stark-fri-poseidon-x7-goldilocks-6x64-v1"),
    ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255_SHA3_512(
        "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
    ),
    ANONYMOUS_PGC_P256("anonymous-pgc-p256"),
    IROHA_VERANGE_P256("iroha-verange-p256"),
    VEGA_NEUTRON_NOVA_SPARTAN_HYRAX_T256("vega-neutron-nova-spartan-hyrax-t256"),
    JINDO_POLYNOMIAL_COMMITMENT("jindo-polynomial-commitment"),
    HALO2_IPA_PASTA("halo2-ipa-pasta"),
    FCMP_PLUS_PLUS_CURVE_TREE_BULLETPROOFS("fcmp-plus-plus-curve-tree-bulletproofs"),
    LANTERN_LNP22_MODULE_LINEAR_NORM("lantern-lnp22-module-linear-norm"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyProofSystemIdV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown canonical privacy proof-system id")
    }
}

/** Closed first-release native-engine identity in canonical Norito discriminant order. */
enum class PrivacyEngineIdV1(val canonicalLabel: String) {
    NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1(
        "native-goldilocks-poseidon-x7-stark-fri-6x64-v1",
    ),
    NATIVE_ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255(
        "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
    ),
    NATIVE_ANONYMOUS_PGC_P256("native-anonymous-pgc-p256"),
    NATIVE_VERANGE_P256("native-verange-p256"),
    NATIVE_VEGA("native-vega"),
    NATIVE_JINDO("native-jindo"),
    NATIVE_HALO2_ORCHARD("native-halo2-orchard"),
    NATIVE_FCMP_PLUS_PLUS("native-fcmp-plus-plus"),
    NATIVE_LANTERN_LNP22("native-lantern-lnp22"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyEngineIdV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown canonical privacy engine id")
    }
}

/** Closed first-release protocol identity in canonical Norito discriminant order. */
enum class PrivacyProtocolIdV1(
    val canonicalLabel: String,
    val expectedProofSystem: PrivacyProofSystemIdV1,
    val expectedEngine: PrivacyEngineIdV1,
) {
    ZK_ACE_PQ_AUTHORIZATION_V1(
        "zk-ace-pq-authorization-v1",
        PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1,
        PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1,
    ),
    ANONYMOUS_PGC_K_OUT_OF_N_V1(
        "anonymous-pgc-k-out-of-n-v1",
        PrivacyProofSystemIdV1.ANONYMOUS_PGC_P256,
        PrivacyEngineIdV1.NATIVE_ANONYMOUS_PGC_P256,
    ),
    VERANGE_TRANSPARENT_RANGE_V1(
        "verange-transparent-range-v1",
        PrivacyProofSystemIdV1.IROHA_VERANGE_P256,
        PrivacyEngineIdV1.NATIVE_VERANGE_P256,
    ),
    IROHA_ZK_AMS_V1(
        "iroha-zk-ams-v1",
        PrivacyProofSystemIdV1.ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255_SHA3_512,
        PrivacyEngineIdV1.NATIVE_ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255,
    ),
    VEGA_EXISTING_CREDENTIAL_ZK_V1(
        "vega-existing-credential-zk-v1",
        PrivacyProofSystemIdV1.VEGA_NEUTRON_NOVA_SPARTAN_HYRAX_T256,
        PrivacyEngineIdV1.NATIVE_VEGA,
    ),
    IROHA_ZK_X509_STARK_P256_V1(
        "iroha-zk-x509-stark-p256-v1",
        PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1,
        PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1,
    ),
    IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1(
        "iroha-jindo-polynomial-commitment-v1",
        PrivacyProofSystemIdV1.JINDO_POLYNOMIAL_COMMITMENT,
        PrivacyEngineIdV1.NATIVE_JINDO,
    ),
    IROHA_BOOTLE_LANTERN_ANONCRED_V1(
        "iroha-bootle-lantern-anoncred-v1",
        PrivacyProofSystemIdV1.LANTERN_LNP22_MODULE_LINEAR_NORM,
        PrivacyEngineIdV1.NATIVE_LANTERN_LNP22,
    ),
    ORCHARD_HALO2_ACTIONS_V1(
        "orchard-halo2-actions-v1",
        PrivacyProofSystemIdV1.HALO2_IPA_PASTA,
        PrivacyEngineIdV1.NATIVE_HALO2_ORCHARD,
    ),
    MONERO_FCMP_PLUS_PLUS_V1(
        "monero-fcmp-plus-plus-v1",
        PrivacyProofSystemIdV1.FCMP_PLUS_PLUS_CURVE_TREE_BULLETPROOFS,
        PrivacyEngineIdV1.NATIVE_FCMP_PLUS_PLUS,
    ),
    IROHA_IVM_PRIVATE_NOTE_STARK_V1(
        "iroha-ivm-private-note-stark-v1",
        PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1,
        PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1,
    ),
    PQ_MASP_STARK_V1(
        "pq-masp-stark-v1",
        PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1,
        PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1,
    ),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyProtocolIdV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown canonical privacy protocol id")
    }
}
