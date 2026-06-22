package org.hyperledger.iroha.sdk.core.model.zk

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class VerifyingKeyBackendTagTest {

    @Test
    fun `parse resolves exact norito value`() {
        assertEquals(VerifyingKeyBackendTag.GROTH16, VerifyingKeyBackendTag.parse("groth16"))
    }

    @Test
    fun `parse rejects leading and trailing whitespace`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyBackendTag.parse("  halo2-bn254  ")
        }
    }

    @Test
    fun `parse rejects case mutated norito values`() {
        for (value in listOf("STARK", "Halo2-Ipa-Pasta")) {
            assertFailsWith<IllegalArgumentException>(value) {
                VerifyingKeyBackendTag.parse(value)
            }
        }
    }

    @Test
    fun `parse resolves fail-closed pending production backend tags`() {
        val cases = listOf(
            "halo2-ipa-orchard" to VerifyingKeyBackendTag.HALO2_IPA_ORCHARD,
            "groth16-bls12-377" to VerifyingKeyBackendTag.GROTH16_BLS12_377,
            "fcmp-plus-plus-curve-tree" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "lattice-pcs-sis" to VerifyingKeyBackendTag.LATTICE_PCS_SIS,
            "miden-stark" to VerifyingKeyBackendTag.MIDEN_STARK,
            "aztec-plonkish-private-kernel" to VerifyingKeyBackendTag.AZTEC_PLONKISH_PRIVATE_KERNEL,
            "pq-masp-stark-fri" to VerifyingKeyBackendTag.PQ_MASP_STARK_FRI,
            "anonymous-pgc" to VerifyingKeyBackendTag.ANONYMOUS_PGC,
            "verange" to VerifyingKeyBackendTag.VERANGE,
            "zkat" to VerifyingKeyBackendTag.ZKAT,
            "recursive-anonymous-admission" to VerifyingKeyBackendTag.RECURSIVE_ANONYMOUS_ADMISSION,
            "vega-existing-credential-zk" to VerifyingKeyBackendTag.VEGA_EXISTING_CREDENTIAL_ZK,
            "silent-threshold-anoncred" to VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED,
            "zk-x509" to VerifyingKeyBackendTag.ZK_X509,
            "sis-with-hints" to VerifyingKeyBackendTag.SIS_WITH_HINTS,
        )

        for ((wireName, tag) in cases) {
            assertEquals(tag, VerifyingKeyBackendTag.parse(wireName))
            assertEquals(wireName, tag.noritoValue)
            assertTrue(tag.isPendingProductionBackend)
            assertTrue(VerifyingKeyBackendTag.isPendingProductionBackendLabel(wireName))
        }
    }

    @Test
    fun `catalog aliases classify as exact pending production backends`() {
        val cases = listOf(
            "halo2/ipa/orchard" to VerifyingKeyBackendTag.HALO2_IPA_ORCHARD,
            "orchard" to VerifyingKeyBackendTag.HALO2_IPA_ORCHARD,
            "zcash-orchard" to VerifyingKeyBackendTag.HALO2_IPA_ORCHARD,
            "groth16/bls12-377" to VerifyingKeyBackendTag.GROTH16_BLS12_377,
            "penumbra-masp" to VerifyingKeyBackendTag.GROTH16_BLS12_377,
            "halo2/ipa/penumbra" to VerifyingKeyBackendTag.GROTH16_BLS12_377,
            "halo2/ipa/masp" to VerifyingKeyBackendTag.GROTH16_BLS12_377,
            "monero-fcmp++" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "fcmp++" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "fcmp-plus-plus-curve-tree" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "halo2/ipa/monero" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "halo2/ipa/curve-tree" to VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE,
            "jindo-lattice-pcs-zk" to VerifyingKeyBackendTag.LATTICE_PCS_SIS,
            "verange-transparent-range" to VerifyingKeyBackendTag.VERANGE,
            "anonymous-pgc-k-out-of-n" to VerifyingKeyBackendTag.ANONYMOUS_PGC,
            "stark/fri/miden" to VerifyingKeyBackendTag.MIDEN_STARK,
            "aztec/private-kernel" to VerifyingKeyBackendTag.AZTEC_PLONKISH_PRIVATE_KERNEL,
            "stark/fri/pq-masp-stark-fri" to VerifyingKeyBackendTag.PQ_MASP_STARK_FRI,
            "post-quantum-masp" to VerifyingKeyBackendTag.PQ_MASP_STARK_FRI,
            "anonymous-pgc-k-out-of-n-v1" to VerifyingKeyBackendTag.ANONYMOUS_PGC,
            "ve-range-transparent-range-v1" to VerifyingKeyBackendTag.VERANGE,
            "zkAt policy-private authenticator" to VerifyingKeyBackendTag.ZKAT,
            "zk-at-policy-private-authenticator" to VerifyingKeyBackendTag.ZKAT,
            "zk-ams-recursive-admission-v0" to VerifyingKeyBackendTag.RECURSIVE_ANONYMOUS_ADMISSION,
            "vega-existing-credential-zk-v0" to VerifyingKeyBackendTag.VEGA_EXISTING_CREDENTIAL_ZK,
            "threshold-anonymous-credentials" to VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED,
            "silent-threshold-anonymous-credential" to VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED,
            "zkvm-x509-identity" to VerifyingKeyBackendTag.ZK_X509,
            "lattice-anonymous-credentials" to VerifyingKeyBackendTag.SIS_WITH_HINTS,
        )

        for ((label, tag) in cases) {
            assertEquals(tag, VerifyingKeyBackendTag.fromCatalogLabel(label))
            assertTrue(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    @Test
    fun `adversarial pending backend label splices stay unsupported`() {
        val cases = listOf(
            "halo2/ipa/orchard/dev-fixture",
            "stark/fri/miden/claimed-production",
            "anonymous-pgc-k-out-of-n-v1-production",
            "sis-hints-anoncred-pq-v0-devfixture",
            "groth16/bls12-377/../../prod",
            "post-quantum-masp/audit-claimed",
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
        )

        for (label in cases) {
            assertEquals(VerifyingKeyBackendTag.UNSUPPORTED, VerifyingKeyBackendTag.fromCatalogLabel(label))
            assertFalse(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
            assertFailsWith<IllegalArgumentException> {
                VerifyingKeyBackendTag.parse(label)
            }
        }
    }

    @Test
    fun `supported backend aliases remain non pending`() {
        val cases = listOf(
            "halo2/ipa" to VerifyingKeyBackendTag.HALO2_IPA_PASTA,
            "halo2/ipa/pasta" to VerifyingKeyBackendTag.HALO2_IPA_PASTA,
            "halo2/pasta/ipa/vote-bool" to VerifyingKeyBackendTag.HALO2_IPA_PASTA,
            "halo2/bn254" to VerifyingKeyBackendTag.HALO2_BN254,
            "groth16" to VerifyingKeyBackendTag.GROTH16,
            "groth16/bn254" to VerifyingKeyBackendTag.GROTH16,
            "stark" to VerifyingKeyBackendTag.STARK,
            "stark/fri" to VerifyingKeyBackendTag.STARK,
            "stark/fri/sha256-goldilocks" to VerifyingKeyBackendTag.STARK,
            "stark/fri/poseidon2-goldilocks" to VerifyingKeyBackendTag.STARK,
            "stark/fri/sha256_goldilocks.v1" to VerifyingKeyBackendTag.STARK,
            "" to VerifyingKeyBackendTag.UNSUPPORTED,
            "unknown-backend" to VerifyingKeyBackendTag.UNSUPPORTED,
            "unknown/privacy/backend" to VerifyingKeyBackendTag.UNSUPPORTED,
            null to VerifyingKeyBackendTag.UNSUPPORTED,
        )

        for ((label, tag) in cases) {
            assertEquals(tag, VerifyingKeyBackendTag.fromCatalogLabel(label))
            assertFalse(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    @Test
    fun `catalog aliases reject non ascii confusable labels before compacting`() {
        val labels = listOf(
            "halo2\uFF0Fipa",
            "halo2/\u200Bipa",
            "h\u0430lo2/ipa",
            "stark\uFF0Ffri/sha256-goldilocks",
            "stark/fri/\u200Bsha256-goldilocks",
            "st\u0430rk/fri/sha256-goldilocks",
        )

        for (label in labels) {
            assertEquals(VerifyingKeyBackendTag.UNSUPPORTED, VerifyingKeyBackendTag.fromCatalogLabel(label))
            assertFalse(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    @Test
    fun `production verifier backend classifier admits supported labels`() {
        val supported = listOf(
            "halo2/ipa",
            "halo2/ipa:ivm-execution-v1",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/kagemusha-folded-v1",
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kagemusha-recursive-compact-v1",
            "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        )

        for (backend in supported) {
            assertTrue(VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend), backend)
            assertEquals(backend, VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend))
        }
    }

    @Test
    fun `production verifier backend classifier rejects non canonical whitespace labels`() {
        val unsafe = listOf(
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2\uFF0Fipa",
            "halo2/\u200Bipa",
            "h\u0430lo2/ipa",
            "HALO2/IPA",
            "stark/FRI",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa:",
            "halo2/ipa.",
            "halo2/ipa/.ivm-execution-v1",
            "halo2/ipa:ivm..execution-v1",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "halo2/ipa/penumbra",
            "halo2/ipa/masp",
            "halo2/ipa/monero",
            "halo2/ipa/curve-tree",
            "halo2/pasta/tiny-add",
            "halo2/ipa/tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/tiny-commit-open",
            "halo2/pasta/anon-transfer-2x2",
            "halo2/ipa/anon-transfer-2x2",
            "halo2/ipa:anon-transfer-2x2",
            "halo2/pasta/anon-transfer-2x2-merkle2",
            "halo2/ipa/anon-transfer-2x2-merkle8",
            "halo2/ipa:anon-transfer-2x2-merkle16",
            "halo2/pasta/vote-bool-commit",
            "halo2/ipa/vote-bool-commit",
            "halo2/ipa:vote-bool-commit",
            "halo2/pasta/vote-bool-commit-merkle2",
            "halo2/ipa/vote-bool-commit-merkle8",
            "halo2/ipa:vote-bool-commit-merkle16",
            "halo2/pasta/asset-hidden-transfer-public-test",
            "halo2/ipa/asset-hidden-transfer-public-test",
            "halo2/ipa:asset-hidden-transfer-public-test",
            "stark/fri/latest",
            "stark/fri/attestation",
            "stark/fri/contest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/audit-proof-v1",
            "stark/fri/sha256 goldilocks",
            "stark/fri/sha256+goldilocks",
            "halo2/ipa+mock",
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "halo2/ipa/orchard:production-ready",
            "orchard:mainnet-ready",
            "penumbra-masp:external-security-review",
            "jindo-lattice-pcs-zk:release-ready",
            "miden-stark:dev-fixture",
            "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/todo",
            "stark/fri/t-o-d-o",
            "stark/fri/draft-only",
            "stark/fri/d-r-a-f-t",
            "stark/fri/pending-audit",
            "stark/fri/replace-before-mainnet",
            "stark/fri/not-production-ready",
            "stark/fri/placeholder",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:todo-proof",
            "halo2/ipa:t-o-d-o-proof",
            "halo2/ipa:draft-proof",
            "halo2/ipa:d-r-a-f-t-proof",
            "halo2/ipa:pending-audit",
            "halo2/ipa:replace-before-production",
            "halo2/ipa:not-for-production",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
        )

        for (backend in unsafe) {
            assertFalse(VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend))
            val error = assertFailsWith<IllegalArgumentException>(backend) {
                VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend)
            }
            val expected = when {
                backend.isBlank() -> "must not be blank"
                backend.trim() != backend -> "surrounding whitespace"
                else -> "unsupported production verifier backend"
            }
            assertTrue(error.message?.contains(expected) == true, error.message ?: "")
        }
    }

    @Test
    fun `parse throws on unknown value`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyBackendTag.parse("nonexistent")
        }
    }
}
