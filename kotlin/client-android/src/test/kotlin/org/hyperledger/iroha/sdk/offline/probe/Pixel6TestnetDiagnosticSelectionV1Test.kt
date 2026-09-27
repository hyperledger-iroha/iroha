// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.junit.jupiter.api.Test

class Pixel6TestnetDiagnosticSelectionV1Test {
    private val network = ByteArray(32) { 0x71 }
    private val owner = "account\u0000actor\u0000runtime\u0000".toByteArray(Charsets.UTF_8)

    private fun field(tag: Int): ByteArray = MessageDigest.getInstance("SHA-256").digest(
        "iroha:kagemusha:v1:pixel6-testnet-diagnostic-selection\u0000"
            .toByteArray(Charsets.US_ASCII) + byteArrayOf(tag.toByte()) + network +
            byteArrayOf(owner.size.toByte(), (owner.size ushr 8).toByte()) + owner,
    )

    private fun canonicalFrame(): ByteArray = ByteBuffer.allocate(460)
        .order(ByteOrder.LITTLE_ENDIAN).apply {
            put("iroha:kagemusha:v1:hardware-transition-selection\u0000"
                .toByteArray(Charsets.US_ASCII))
            putLong(403L)
            putShort(1.toShort())
            (1..4).forEach { put(field(it)) }
            put(network)
            put(field(5))
            put(field(6))
            putLong(1L)
            put(field(7))
            putLong(1L)
            put(5.toByte())
            put(field(8))
            put(ByteArray(80))
            put(1.toByte())
            put(ByteArray(15))
        }.array()

    @Test fun rustCanonicalDiagnosticVectorIsAcceptedAndBoundToOwner() {
        val frame = canonicalFrame()
        assertEquals("6c9a8f1aea1d86de62939c1ef3e20fc7ed9832fe833a41fbe12e5e0f60198257",
            MessageDigest.getInstance("SHA-256").digest(frame).joinToString("") {
                "%02x".format(it.toInt() and 0xff)
            })
        val endpoint = object : Pixel6TestnetDiagnosticSelectionEndpointV1 {
            override fun contract(): IntArray = intArrayOf(1, 32, 2048, 460)
            override fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray {
                assertContentEquals(network, networkId)
                assertContentEquals(owner, ownerScope)
                return frame.copyOf()
            }
        }
        val source = Pixel6TestnetDiagnosticSelectionV1.openEndpoint(endpoint)
        assertContentEquals(frame, source.create(network, owner))
        val changed = frame.copyOf().also { it[59] = (it[59].toInt() xor 1).toByte() }
        assertFailsWith<IllegalStateException> {
            Pixel6TestnetDiagnosticSelectionV1.openEndpoint(object : Pixel6TestnetDiagnosticSelectionEndpointV1 {
                override fun contract(): IntArray = intArrayOf(1, 32, 2048, 460)
                override fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray = changed
            }).create(network, owner)
        }
    }

    @Test fun incompatibleOrUnavailableNativeConstructorFailsClosed() {
        for (word in 0 until 4) {
            assertFailsWith<IllegalStateException> {
                Pixel6TestnetDiagnosticSelectionV1.openEndpoint(
                    object : Pixel6TestnetDiagnosticSelectionEndpointV1 {
                        override fun contract(): IntArray = intArrayOf(1, 32, 2048, 460)
                            .also { it[word] += 1 }
                        override fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray? = null
                    },
                )
            }
        }
        val unavailable = Pixel6TestnetDiagnosticSelectionV1.openEndpoint(
            object : Pixel6TestnetDiagnosticSelectionEndpointV1 {
                override fun contract(): IntArray = intArrayOf(1, 32, 2048, 460)
                override fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray? = null
            },
        )
        assertFailsWith<IllegalStateException> { unavailable.create(network, owner) }
        assertFailsWith<IllegalArgumentException> { unavailable.create(ByteArray(32), owner) }
    }
}
