// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaTestnetStateProofObservationV1Test {
    @Test
    fun `contract drift and missing symbol fail before accepting a proof`() {
        for (word in 0 until 4) {
            val drift = Endpoint().apply { contractWords[word] += 1 }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetStateProofObservationV1.openEndpoint(drift)
            }
            assertEquals(0, drift.observeCalls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetStateProofObservationV1.openEndpoint(
                Endpoint().apply { missingContract = true },
            )
        }
    }

    @Test
    fun `bounds reject input before native dispatch`() {
        val endpoint = Endpoint()
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(endpoint)
        listOf(ByteArray(0), ByteArray(4097)).forEach { public ->
            assertFailsWith<IllegalArgumentException> { observer.observeStateProof(public, byteArrayOf(1)) }
        }
        listOf(ByteArray(0), ByteArray(6529)).forEach { proof ->
            assertFailsWith<IllegalArgumentException> { observer.observeStateProof(byteArrayOf(1), proof) }
        }
        assertEquals(0, endpoint.observeCalls)
        observer.observeStateProof(ByteArray(4096), ByteArray(6528))
        assertEquals(1, endpoint.observeCalls)
    }

    @Test
    fun `stock missing owner and rejected proofs never return diagnostic bytes`() {
        val endpoint = Endpoint().apply { status = -312 }
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(endpoint)
        val unavailable = assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeStateProof(byteArrayOf(1), byteArrayOf(2))
        }
        assertEquals(-312, unavailable.status)
        assertTrue(unavailable.message!!.contains("owner is unavailable"))
        endpoint.status = -311
        val rejected = assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeStateProof(byteArrayOf(1), byteArrayOf(2))
        }
        assertEquals(-311, rejected.status)
    }

    @Test
    fun `success reads exact native length from preallocated direct buffer`() {
        val endpoint = Endpoint()
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(endpoint)
        endpoint.status = 3
        assertContentEquals(byteArrayOf(7, 8, 9), observer.observeStateProof(byteArrayOf(1), byteArrayOf(2)))
        assertTrue(endpoint.sawDirectOutput)
        assertEquals(256, endpoint.outputCapacity)
        endpoint.status = 0
        assertEquals(0, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeStateProof(byteArrayOf(1), byteArrayOf(2))
        }.status)
        endpoint.status = 257
        assertEquals(257, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            observer.observeStateProof(byteArrayOf(1), byteArrayOf(2))
        }.status)
    }

    @Test
    fun `native endpoint cannot mutate caller proof inputs`() {
        val endpoint = Endpoint().apply { mutateInputs = true }
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(endpoint)
        val publicInputs = byteArrayOf(1, 2, 3)
        val pairedProof = byteArrayOf(4, 5, 6)
        assertContentEquals(byteArrayOf(7, 8, 9),
            observer.observeStateProof(publicInputs, pairedProof))
        assertContentEquals(byteArrayOf(1, 2, 3), publicInputs)
        assertContentEquals(byteArrayOf(4, 5, 6), pairedProof)
        assertContentEquals(byteArrayOf(1, 2, 3), endpoint.seenPublicInputs)
        assertContentEquals(byteArrayOf(4, 5, 6), endpoint.seenPairedProof)
    }

    @Test
    fun `missing JNI observe symbol fails closed`() {
        val endpoint = Endpoint().apply { missingObserve = true }
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(endpoint)
        assertFailsWith<IllegalStateException> {
            observer.observeStateProof(byteArrayOf(1), byteArrayOf(2))
        }
    }

    private class Endpoint : KagemushaTestnetStateProofObservationEndpointV1 {
        val contractWords = intArrayOf(1, 4096, 6528, 256)
        var status = 3
        var observeCalls = 0
        var missingContract = false
        var missingObserve = false
        var sawDirectOutput = false
        var outputCapacity = 0
        var mutateInputs = false
        var seenPublicInputs: ByteArray? = null
        var seenPairedProof: ByteArray? = null

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract symbol")
            return contractWords.copyOf()
        }

        override fun observe(
            publicInputsArchive: ByteArray,
            pairedProofArchive: ByteArray,
            output: ByteBuffer,
        ): Int {
            if (missingObserve) throw UnsatisfiedLinkError("missing observe symbol")
            observeCalls++
            seenPublicInputs = publicInputsArchive.copyOf()
            seenPairedProof = pairedProofArchive.copyOf()
            if (mutateInputs) {
                publicInputsArchive[0] = 0
                pairedProofArchive[0] = 0
            }
            sawDirectOutput = output.isDirect
            outputCapacity = output.capacity()
            output.put(byteArrayOf(7, 8, 9))
            return status
        }
    }
}
