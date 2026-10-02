// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Public structural specimens exercise original-byte parsing without authenticating evidence. */
class KagemushaAppAttestOriginalCounterV1Test {
    @Test fun `original counter preserves unsigned values and either canonical key order`() {
        for (counter in listOf(0u, 11u, 0x80000000u, UInt.MAX_VALUE)) {
            for (reverse in listOf(false, true)) {
                assertEquals(counter, read(assertion(auth(counter), reverse = reverse)))
            }
        }
    }

    @Test fun `release extensions retain exact native categories flags and UTF8 bounds`() {
        for (category in listOf(1, 2, 3, 4, 5, 6, 10)) {
            for (flag in listOf(0x40, 0xc0)) {
                for (reverse in listOf(false, true)) {
                    assertEquals(11u, read(assertion(auth(11u, flag) + extensions(category, "版2", reverse))))
                }
            }
        }
        assertEquals(11u, read(assertion(auth(11u, 0xc0) + extensions(10, "x".repeat(128)))))
        for (category in listOf(0, 7, 9, 11, -1)) {
            assertFailsWith<IllegalArgumentException> { read(assertion(auth(11u, 0xc0) + extensions(category, "1"))) }
        }
        for (version in listOf("", "a\u0000b", "x".repeat(129))) {
            assertFailsWith<IllegalArgumentException> { read(assertion(auth(11u, 0xc0) + extensions(1, version))) }
        }
        val duplicate = byteArrayOf(0xa2.toByte()) + text("bundleVersion") + text("1") + text("bundleVersion") + text("2")
        assertFailsWith<IllegalArgumentException> { read(assertion(auth(11u, 0xc0) + duplicate)) }
        assertFailsWith<IllegalArgumentException> { read(assertion(auth(11u, 0xc0) + extensions(1, "1") + byteArrayOf(0))) }
        val category = text("validationCategory") + bytes(byteArrayOf(1, 0, 0, 0))
        for (extension in listOf(
            byteArrayOf(0xa2.toByte()) + category + category,
            byteArrayOf(0xa2.toByte()) + category + text("unknown") + text("1"),
            byteArrayOf(0xa2.toByte()) + category + text("bundleVersion") + byteArrayOf(0x61, 0xff.toByte()),
            byteArrayOf(0xa2.toByte()) + text("validationCategory") + bytes(ByteArray(3)) + text("bundleVersion") + text("1"),
        )) assertFailsWith<IllegalArgumentException> { read(assertion(auth(11u, 0xc0) + extension)) }
    }

    @Test fun `malformed originals reject truncation tails duplicate keys flags and noncanonical CBOR`() {
        val valid = assertion(auth(11u))
        for (length in valid.indices) assertFailsWith<IllegalArgumentException> { read(valid.copyOf(length)) }
        val authEntry = text("authenticatorData") + bytes(auth(11u))
        val signatureEntry = text("signature") + bytes(signature())
        val malformed = listOf(
            valid + byteArrayOf(0), ByteArray(312),
            byteArrayOf(0xa2.toByte()) + authEntry + authEntry,
            byteArrayOf(0xa2.toByte()) + signatureEntry + signatureEntry,
            byteArrayOf(0xa2.toByte()) + text("unknown") + bytes(auth(11u)) + signatureEntry,
            byteArrayOf(0xbf.toByte()) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xb8.toByte(), 2) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xb9.toByte(), 0, 2) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xba.toByte(), 0, 0, 0, 2) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xbb.toByte()) + ByteArray(8) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xb9.toByte(), 1, 0) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xba.toByte(), 0, 1, 0, 0) + valid.copyOfRange(1, valid.size),
            byteArrayOf(0xa2.toByte(), 0x78, 17) + valid.copyOfRange(2, valid.size),
            byteArrayOf(0xa2.toByte(), 0x61, 0xff.toByte()) + bytes(auth(11u)) + signatureEntry,
            assertion(auth(11u).also { it.fill(0, 0, 32) }),
            assertion(auth(11u, 0)), assertion(auth(11u, 0xc0)),
            assertion(auth(11u).copyOf(36)), assertion(auth(11u).copyOf(207)),
            assertion(auth(11u), signature = ByteArray(7)),
            assertion(auth(11u), signature = ByteArray(73)),
        )
        malformed.forEach { original -> assertFailsWith<IllegalArgumentException> { read(original) } }
    }

    private fun read(original: ByteArray) = KagemushaAppAttestOriginalCounterV1.read(original)
    private fun auth(counter: UInt, flags: Int = 0x40) = ByteArray(37) { 0x42 }.also {
        it[32] = flags.toByte()
        ByteBuffer.wrap(it, 33, 4).putInt(counter.toInt())
    }
    private fun signature() = byteArrayOf(0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01)
    private fun assertion(auth: ByteArray, reverse: Boolean = false, signature: ByteArray = signature()): ByteArray {
        val fields = listOf(text("authenticatorData") + bytes(auth), text("signature") + bytes(signature))
        return byteArrayOf(0xa2.toByte()) + (if (reverse) fields.reversed() else fields).reduce { a, b -> a + b }
    }
    private fun extensions(category: Int, version: String, reverse: Boolean = false): ByteArray {
        val fields = listOf(text("validationCategory") + bytes(byteArrayOf(category.toByte(), 0, 0, 0)),
            text("bundleVersion") + text(version))
        return byteArrayOf(0xa2.toByte()) + (if (reverse) fields.reversed() else fields).reduce { a, b -> a + b }
    }
    private fun text(value: String) = field(3, value.toByteArray(Charsets.UTF_8))
    private fun bytes(value: ByteArray) = field(2, value)
    private fun field(major: Int, value: ByteArray): ByteArray =
        (if (value.size < 24) byteArrayOf(((major shl 5) or value.size).toByte())
        else byteArrayOf(((major shl 5) or 24).toByte(), value.size.toByte())) + value
}
