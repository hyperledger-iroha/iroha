package org.hyperledger.iroha.sdk.core.model.zk

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class VerifyingKeyBackendTagTest {

    @Test
    fun `parse resolves exact norito value`() {
        assertEquals(VerifyingKeyBackendTag.GROTH16, VerifyingKeyBackendTag.parse("groth16"))
    }

    @Test
    fun `parse resolves with leading and trailing whitespace`() {
        assertEquals(VerifyingKeyBackendTag.HALO2_BN254, VerifyingKeyBackendTag.parse("  halo2-bn254  "))
    }

    @Test
    fun `parse is case insensitive`() {
        assertEquals(VerifyingKeyBackendTag.STARK, VerifyingKeyBackendTag.parse("STARK"))
        assertEquals(VerifyingKeyBackendTag.HALO2_IPA_PASTA, VerifyingKeyBackendTag.parse("Halo2-Ipa-Pasta"))
    }

    @Test
    fun `parse throws on unknown value`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyBackendTag.parse("nonexistent")
        }
    }
}
