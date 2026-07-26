// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaP256CodecTest {
    @Test
    fun `uncompressed public key is fixed valid and defensive`() {
        val source = hex(P256_GENERATOR)
        val key = KagemushaDevicePublicKeyV2(source)
        source.fill(0)
        assertContentEquals(hex(P256_GENERATOR), key.sec1Bytes())

        val returned = key.sec1Bytes()
        returned.fill(0)
        assertContentEquals(hex(P256_GENERATOR), key.sec1Bytes())
        assertEquals(KagemushaDevicePublicKeyV2(hex(P256_GENERATOR)), key)

        for (invalid in listOf(
            hex(P256_GENERATOR).copyOf(64),
            hex(P256_GENERATOR).copyOf(66),
            hex(P256_GENERATOR).also { it[0] = 0x02 },
            hex(P256_GENERATOR).also { it[0] = 0x03 },
            hex(P256_GENERATOR).also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() },
            ByteArray(65).also { it[0] = 0x04 },
        )) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaDevicePublicKeyV2(invalid)
            }
        }
    }

    @Test
    fun `raw signature requires exact in-range low-S scalars`() {
        val raw = raw(BigInteger.ONE, BigInteger.ONE)
        val signature = KagemushaDeviceSignatureV2(raw)
        raw.fill(0)
        assertContentEquals(raw(BigInteger.ONE, BigInteger.ONE), signature.rawBytes())
        assertContentEquals(
            byteArrayOf(0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01),
            signature.strictDer(),
        )

        val invalid = listOf(
            ByteArray(63),
            ByteArray(65),
            raw(BigInteger.ZERO, BigInteger.ONE),
            raw(BigInteger.ONE, BigInteger.ZERO),
            raw(ORDER, BigInteger.ONE),
            raw(BigInteger.ONE, ORDER),
            raw(BigInteger.ONE, HALF_ORDER.add(BigInteger.ONE)),
        )
        invalid.forEach {
            assertFailsWith<IllegalArgumentException> {
                KagemushaDeviceSignatureV2(it)
            }
        }
    }

    @Test
    fun `strict DER conversion normalizes high-S and round trips minimally`() {
        val low = raw(BigInteger.valueOf(128), BigInteger.ONE)
        val expectedDer = byteArrayOf(
            0x30, 0x07, 0x02, 0x02, 0x00, 0x80.toByte(), 0x02, 0x01, 0x01,
        )
        assertContentEquals(expectedDer, KagemushaP256Codec.strictDerFromRawLowS(low))
        assertContentEquals(low, KagemushaP256Codec.rawLowSFromStrictDer(expectedDer))

        val highDer = der(BigInteger.ONE, ORDER.subtract(BigInteger.ONE))
        val normalized = KagemushaP256Codec.rawLowSFromStrictDer(highDer)
        assertContentEquals(raw(BigInteger.ONE, BigInteger.ONE), normalized)
        assertContentEquals(
            normalized,
            KagemushaDeviceSignatureV2.fromStrictDer(highDer).rawBytes(),
        )
    }

    @Test
    fun `strict DER rejects alternate malformed and out-of-range encodings`() {
        val valid = der(BigInteger.ONE, BigInteger.ONE)
        val cases = listOf(
            ByteArray(0),
            valid.copyOf(valid.size - 1),
            valid + 0,
            byteArrayOf(0x30, 0x81.toByte(), 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01),
            byteArrayOf(0x30, 0x07, 0x02, 0x02, 0x00, 0x01, 0x02, 0x01, 0x01),
            byteArrayOf(0x30, 0x06, 0x02, 0x01, 0x80.toByte(), 0x02, 0x01, 0x01),
            byteArrayOf(0x30, 0x06, 0x02, 0x01, 0x00, 0x02, 0x01, 0x01),
            byteArrayOf(0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00),
            der(ORDER, BigInteger.ONE),
            der(BigInteger.ONE, ORDER),
            byteArrayOf(0x31, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01),
            byteArrayOf(0x30, 0x06, 0x03, 0x01, 0x01, 0x02, 0x01, 0x01),
        )
        cases.forEach { encoded ->
            assertFailsWith<IllegalArgumentException>(encoded.joinToString()) {
                KagemushaP256Codec.rawLowSFromStrictDer(encoded)
            }
        }
    }

    companion object {
        private const val P256_GENERATOR =
            "04" +
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        private val ORDER =
            BigInteger("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16)
        private val HALF_ORDER = ORDER.shiftRight(1)

        private fun raw(r: BigInteger, s: BigInteger): ByteArray = fixed(r) + fixed(s)

        private fun fixed(value: BigInteger): ByteArray {
            val signed = value.toByteArray()
            val source = if (signed.size == 33 && signed[0].toInt() == 0) {
                signed.copyOfRange(1, signed.size)
            } else {
                signed
            }
            require(source.size <= 32)
            return ByteArray(32).also { source.copyInto(it, 32 - source.size) }
        }

        private fun der(r: BigInteger, s: BigInteger): ByteArray {
            fun integer(value: BigInteger): ByteArray {
                val encoded = value.toByteArray()
                return byteArrayOf(0x02, encoded.size.toByte()) + encoded
            }
            val body = integer(r) + integer(s)
            return byteArrayOf(0x30, body.size.toByte()) + body
        }

        private fun hex(value: String): ByteArray = ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }
}
