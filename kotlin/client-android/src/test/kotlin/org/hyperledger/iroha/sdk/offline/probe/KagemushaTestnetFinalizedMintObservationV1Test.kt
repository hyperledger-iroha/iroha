// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaTestnetFinalizedMintObservationV1Test {
    private fun checkpoint(): ByteArray = generateSequence(java.io.File(System.getProperty("user.dir"))) { it.parentFile }
        .map { java.io.File(it, "fixtures/sumeragi/native-finality/height-2-checkpoint.nrt") }
        .first { it.isFile }.readBytes()

    @Test
    fun `every ABI word is pinned before observation`() {
        for (word in 0 until 8) {
            val endpoint = Endpoint().apply { contractWords[word] += 1 }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetFinalizedMintObservationV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.calls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetFinalizedMintObservationV1.openEndpoint(
                Endpoint().apply { missingContract = true },
            )
        }
    }

    @Test
    fun `invalid original operation and oversized archives never cross JNI`() {
        val endpoint = Endpoint()
        val observer = KagemushaTestnetFinalizedMintObservationV1.openEndpoint(endpoint)
        val id = ByteArray(32) { 1 }
        val network = ByteArray(32) { 1 }
        val checkpoint = checkpoint()
        val value = byteArrayOf(1)
        assertFailsWith<IllegalArgumentException> {
            observer.observeFinalizedMint(ByteArray(32), value, network, checkpoint, value, value)
        }
        assertFailsWith<IllegalArgumentException> {
            observer.observeFinalizedMint(ByteArray(31), value, network, checkpoint, value, value)
        }
        for (bad in listOf(ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> {
                observer.observeFinalizedMint(id, value, bad, checkpoint, value, value)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            observer.observeFinalizedMint(id, value, network, byteArrayOf(), value, value)
        }
        assertFailsWith<IllegalArgumentException> {
            observer.observeFinalizedMint(id, value, network, ByteArray(68 * 1024 * 1024 + 1), value, value)
        }
        for (position in 0 until 3) {
            val inputs = arrayOf(value, value, value)
            inputs[position] = ByteArray(intArrayOf(150_995_969, 4097, 6529)[position])
            assertFailsWith<IllegalArgumentException> {
                observer.observeFinalizedMint(id, inputs[0], network, checkpoint,
                    inputs[1], inputs[2])
            }
        }
        assertEquals(0, endpoint.calls)
    }

    @Test
    fun `stock missing durable owner and invalid proof fail closed`() {
        val endpoint = Endpoint().apply { status = -312 }
        val observer = KagemushaTestnetFinalizedMintObservationV1.openEndpoint(endpoint)
        val unavailable = assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeFinalizedMint(ByteArray(32) { 1 }, byteArrayOf(2),
                ByteArray(32) { 1 }, checkpoint(), byteArrayOf(4), byteArrayOf(5))
        }
        assertEquals(-312, unavailable.status)
        assertTrue(unavailable.message!!.contains("owner is unavailable"))
        endpoint.status = -311
        assertEquals(-311, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeFinalizedMint(ByteArray(32) { 1 }, byteArrayOf(2),
                ByteArray(32) { 1 }, checkpoint(), byteArrayOf(4), byteArrayOf(5))
        }.status)
    }

    @Test
    fun `complete direct output is returned without mutating caller arrays`() {
        val endpoint = Endpoint().apply { mutateInputs = true }
        val observer = KagemushaTestnetFinalizedMintObservationV1.openEndpoint(endpoint)
        val inputs = arrayOf(ByteArray(32) { 1 }, byteArrayOf(2), ByteArray(32) { 1 },
            checkpoint(), byteArrayOf(4), byteArrayOf(5))
        assertContentEquals(byteArrayOf(7, 8, 9), observer.observeFinalizedMint(
            inputs[0], inputs[1], inputs[2], inputs[3], inputs[4], inputs[5]))
        assertTrue(endpoint.direct)
        assertEquals(512, endpoint.capacity)
        assertContentEquals(inputs[3], endpoint.seen[3])
        inputs.forEachIndexed { index, original -> assertContentEquals(original, endpoint.seen[index]) }
        endpoint.status = 0
        assertEquals(0, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeFinalizedMint(inputs[0], inputs[1], inputs[2],
                inputs[3], inputs[4], inputs[5])
        }.status)
        endpoint.status = 513
        assertEquals(513, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeFinalizedMint(inputs[0], inputs[1], inputs[2],
                inputs[3], inputs[4], inputs[5])
        }.status)
    }

    @Test
    fun `missing observe symbol cannot produce an observation`() {
        val observer = KagemushaTestnetFinalizedMintObservationV1.openEndpoint(
            Endpoint().apply { missingObserve = true },
        )
        assertFailsWith<IllegalStateException> {
            observer.observeFinalizedMint(ByteArray(32) { 1 }, byteArrayOf(2),
                ByteArray(32) { 1 }, checkpoint(), byteArrayOf(4), byteArrayOf(5))
        }
    }

    private class Endpoint : KagemushaTestnetFinalizedMintObservationEndpointV1 {
        val contractWords = intArrayOf(1, 32, 150995968, 32, 71303168, 4096, 6528, 512)
        var status = 3
        var calls = 0
        var missingContract = false
        var missingObserve = false
        var mutateInputs = false
        var direct = false
        var capacity = 0
        var seen: Array<ByteArray> = emptyArray()

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract")
            return contractWords.copyOf()
        }

        override fun observe(
            operationId: ByteArray,
            statusJson: ByteArray,
            anchorNetworkId: ByteArray,
            anchorCheckpoint: ByteArray,
            statePublicInputs: ByteArray,
            pairedProof: ByteArray,
            output: ByteBuffer,
        ): Int {
            if (missingObserve) throw UnsatisfiedLinkError("missing observe")
            calls++
            val inputs = arrayOf(operationId, statusJson, anchorNetworkId, anchorCheckpoint,
                statePublicInputs, pairedProof)
            seen = inputs.map(ByteArray::copyOf).toTypedArray()
            if (mutateInputs) inputs.forEach { it.fill(0) }
            direct = output.isDirect
            capacity = output.capacity()
            output.put(byteArrayOf(7, 8, 9))
            return status
        }
    }
}
