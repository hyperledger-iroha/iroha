package org.hyperledger.iroha.sdk.crypto

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class Ed25519PublicKeyAdmissionStrictTest {
    @Test
    fun `admits valid prime-order keys and rejects torsion and malformed encodings`() {
        assertTrue(
            Ed25519PublicKeyAdmission.isValid(
                hex("3B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29"),
            ),
        )

        val invalid = mapOf(
            "all-zero" to "00".repeat(32),
            "small-order identity" to "01" + "00".repeat(31),
            "noncanonical identity" to
                "EEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7F",
            "invalid compressed point" to "02".repeat(32),
            "mixed torsion repeated-11" to "11".repeat(32),
            "mixed torsion base-plus-torsion" to
                "6AEBC0B955CE4A2F1344029986B775E6EA5C40F93F1112B86EC51678EB9DC0FB",
        )
        for ((name, encoded) in invalid) {
            assertFalse(Ed25519PublicKeyAdmission.isValid(hex(encoded)), name)
        }
        assertFalse(Ed25519PublicKeyAdmission.isValid(null))
        assertFalse(Ed25519PublicKeyAdmission.isValid(ByteArray(31)))
    }

    private fun hex(encoded: String): ByteArray {
        require(encoded.length % 2 == 0)
        return ByteArray(encoded.length / 2) { index ->
            encoded.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }
}
