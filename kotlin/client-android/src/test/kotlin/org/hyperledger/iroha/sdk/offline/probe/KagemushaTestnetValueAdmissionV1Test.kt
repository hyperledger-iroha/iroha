// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaTestnetValueAdmissionV1Test {
    @Test
    fun `all native contract words are required`() {
        for (index in 0 until 3) {
            val endpoint = Endpoint().apply { contractWords[index] += 1 }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.calls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetValueAdmissionV1.openEndpoint(Endpoint().apply { missingContract = true })
        }
    }

    @Test
    fun `zero or malformed operation never reaches JNI`() {
        val endpoint = Endpoint()
        val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
        for (id in listOf(ByteArray(0), ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> { admission.admitFinalizedValue(id) }
        }
        assertEquals(0, endpoint.calls)
    }

    @Test
    fun `only a positive bounded direct archive is returned`() {
        val endpoint = Endpoint()
        val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
        val id = ByteArray(32) { 7 }
        assertContentEquals(byteArrayOf(3, 4, 5), admission.admitFinalizedValue(id))
        assertContentEquals(ByteArray(32) { 7 }, id)
        assertTrue(endpoint.direct)
        assertEquals(768, endpoint.capacity)
        assertEquals(1, endpoint.calls)
        endpoint.status = -312
        assertEquals(-312, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.status = 0
        assertEquals(0, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.status = 769
        assertEquals(769, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.missingAdmit = true
        assertFailsWith<IllegalStateException> { admission.admitFinalizedValue(id) }
    }

    private class Endpoint : KagemushaTestnetValueAdmissionEndpointV1 {
        val contractWords = intArrayOf(1, 32, 768)
        var status = 3
        var calls = 0
        var direct = false
        var capacity = 0
        var missingContract = false
        var missingAdmit = false

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract")
            return contractWords.copyOf()
        }

        override fun admit(operationId: ByteArray, output: ByteBuffer): Int {
            if (missingAdmit) throw UnsatisfiedLinkError("missing admit")
            calls++
            operationId.fill(0)
            direct = output.isDirect
            capacity = output.capacity()
            output.put(byteArrayOf(3, 4, 5))
            return status
        }
    }
}
