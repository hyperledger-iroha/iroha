package org.hyperledger.iroha.sdk.core.model.zk

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class VerifyingKeyBackendTagTest {

    private val registry = linkedSetOf(
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
    )

    @Test
    fun `backend enum contains only canonical low-level engines`() {
        assertEquals(
            listOf(
                VerifyingKeyBackendTag.HALO2_IPA_PASTA,
                VerifyingKeyBackendTag.STARK,
            ),
            VerifyingKeyBackendTag.entries,
        )
        assertEquals(
            VerifyingKeyBackendTag.HALO2_IPA_PASTA,
            VerifyingKeyBackendTag.parse("halo2-ipa-pasta"),
        )
        assertEquals(VerifyingKeyBackendTag.STARK, VerifyingKeyBackendTag.parse("stark"))
    }

    @Test
    fun `backend parser rejects aliases retired engines and malformed labels`() {
        for (label in listOf(
            "",
            " halo2-ipa-pasta",
            "halo2-ipa-pasta ",
            "HALO2-IPA-PASTA",
            "Stark",
            "stark ",
            "halo2/ipa",
            "stark/fri",
            "halo2-bn254",
            "groth16",
            "groth16-bls12-377",
            "aztec-plonkish-private-kernel",
            "zkat",
            "silent-threshold-anoncred",
            "unsupported",
            "stark\u0000",
            "st\u0430rk",
        )) {
            assertFailsWith<IllegalArgumentException>(label) {
                VerifyingKeyBackendTag.parse(label)
            }
        }
    }

    @Test
    fun `registry is the exact immutable native allowlist`() {
        assertEquals(15, VerifyingKeyBackendTag.VERIFIER_BACKEND_REGISTRY_LABELS_V1.size)
        assertEquals(registry, VerifyingKeyBackendTag.VERIFIER_BACKEND_REGISTRY_LABELS_V1)
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (VerifyingKeyBackendTag.VERIFIER_BACKEND_REGISTRY_LABELS_V1 as MutableSet<String>)
                .add("stark/fri/latest")
        }
    }

    @Test
    fun `every registry label resolves to one exact engine`() {
        for (label in registry) {
            val expected = if (label.startsWith("halo2/")) {
                VerifyingKeyBackendTag.HALO2_IPA_PASTA
            } else {
                VerifyingKeyBackendTag.STARK
            }
            assertEquals(
                expected,
                VerifyingKeyBackendTag.verifierBackendRegistryTagV1(label),
                label,
            )
            assertTrue(VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(label), label)
            assertEquals(
                label,
                VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(label),
            )
        }
    }

    @Test
    fun `registry rejects aliases retired families and confusables`() {
        val rejected = listOf(
            "",
            "halo2-ipa-pasta",
            "stark",
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "HALO2/IPA",
            "halo2//ipa",
            "halo2/ipa/",
            "halo2/ipa:",
            "halo2/ipa:ivm-execution-v1",
            "halo2/ipa::ivm-execution-v1",
            "halo2/ipa/ivm-execution-v1",
            "halo2/pasta/ipa/ivm-execution-v1",
            "halo2/pasta/ivm_execution_v1",
            "halo2/pasta/ivm-execution-v1/",
            "halo2/pasta/ivm-execution-v1\u0000",
            "halo2/pasta/ipa-pasta-cycle-v1",
            "halo2/ipa-pasta-cycle-v1",
            "halo2/pasta/tiny-add",
            "stark/fri/",
            "STARK/FRI",
            "stark/FRI",
            "stark/fri/latest",
            "stark/fri/sha256-goldilocks/extra",
            "stark/fri/sha256 goldilocks",
            "stark/fri/sha256+goldilocks",
            "stark/fri/sha256-goldilocks\u200B",
            "halo2\uFF0Fipa",
            "halo2/\u200Bipa",
            "h\u0430lo2/ipa",
            "../halo2/ipa",
            "groth16",
            "groth16/bn254",
            "groth16/bls12-377",
            "halo2/bn254",
            "halo2/kzg",
            "kzg/powersoftau",
            "aztec-plonkish-private-kernel",
            "zkat",
            "silent-threshold-anoncred",
            "penumbra-masp",
            "orchard",
            "fcmp++",
            "jindo-lattice-pcs-zk",
            "sis-with-hints",
            "vega-existing-credential-zk-v0",
            "anonymous-pgc-k-out-of-n-v1",
            "stark/fri/dev-fixture",
            "stark/fri/externally-audited",
            "halo2/ipa:production-ready",
            "halo2/ipa:kzg",
        )

        for (label in rejected) {
            assertNull(VerifyingKeyBackendTag.verifierBackendRegistryTagV1(label), label)
            assertFalse(VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(label), label)
            assertFailsWith<IllegalArgumentException>(label) {
                VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(label)
            }
        }
        assertNull(VerifyingKeyBackendTag.verifierBackendRegistryTagV1(null))
        assertFalse(VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(null))
    }

    @Test
    fun `one-byte and structural mutations of every label stay rejected`() {
        for (label in registry) {
            val mutations = linkedSetOf(
                " $label",
                "$label ",
                label.uppercase(),
                "$label/",
                "$label\u0000",
                "$label\u200B",
                label.replaceFirst("/", "//"),
                label.dropLast(1) + if (label.last() == 'x') "y" else "x",
            )
            mutations.remove(label)
            for (mutation in mutations) {
                assertFalse(
                    VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(mutation),
                    "$mutation mutated from $label",
                )
            }
        }
    }
}
