// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.PublicKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/** JCA-to-SEC1 encoding migrated from the retired signature-only suite into the P-256 owner. */
class KagemushaP256CodecPublicKeyTest {
    private fun ecKey(curve: String): ECPublicKey = KeyPairGenerator.getInstance("EC").run {
        initialize(ECGenParameterSpec(curve))
        generateKeyPair().public as ECPublicKey
    }

    private fun fixed(value: BigInteger): ByteArray {
        val bytes = value.toByteArray()
        val trimmed = if (bytes.size == 33) bytes.copyOfRange(1, 33) else bytes
        return ByteArray(32 - trimmed.size) + trimmed
    }

    @Test fun `P-256 JCA keys encode to fixed-width uncompressed SEC1 and round-trip`() {
        repeat(32) {
            val key = ecKey("secp256r1")
            val sec1 = KagemushaP256Codec.uncompressedFromPublicKey(key)
            assertEquals(KagemushaP256Codec.PUBLIC_KEY_BYTES, sec1.size)
            assertEquals(0x04, sec1[0].toInt())
            assertContentEquals(fixed(key.w.affineX), sec1.copyOfRange(1, 33))
            assertContentEquals(fixed(key.w.affineY), sec1.copyOfRange(33, 65))
            assertEquals(key.w, KagemushaP256Codec.publicKeyFromSec1(sec1).w)
        }
    }

    @Test fun `encoded keys verify platform DER signatures through the canonical raw path`() {
        val pair = KeyPairGenerator.getInstance("EC").run {
            initialize(ECGenParameterSpec("secp256r1"))
            generateKeyPair()
        }
        val preimage = "iroha:kagemusha:test".toByteArray(Charsets.US_ASCII)
        val der = Signature.getInstance("SHA256withECDSA").run {
            initSign(pair.private)
            update(preimage)
            sign()
        }
        val sec1 = KagemushaP256Codec.uncompressedFromPublicKey(pair.public)
        assertTrue(KagemushaP256Codec.verifyRawLowS(sec1, preimage, KagemushaP256Codec.rawLowSFromStrictDer(der)))
    }

    @Test fun `keys outside the P-256 domain are rejected`() {
        assertFailsWith<IllegalArgumentException> {
            KagemushaP256Codec.uncompressedFromPublicKey(ecKey("secp384r1"))
        }
        val rsa: PublicKey = KeyPairGenerator.getInstance("RSA").run {
            initialize(1024)
            generateKeyPair().public
        }
        assertFailsWith<IllegalArgumentException> { KagemushaP256Codec.uncompressedFromPublicKey(rsa) }
    }
}
