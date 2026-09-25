// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaTestnetValueCreditV1Test {
    @Test
    fun `credit contract rejects native drift`() {
        for (index in 0 until 3) {
            val endpoint = Endpoint().apply { contractWords[index]++ }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.calls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetValueCreditV1.openEndpoint(Endpoint().apply { missingContract = true })
        }
    }

    @Test
    fun `malformed operation never reaches native ledger`() {
        val endpoint = Endpoint()
        val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
        for (id in listOf(ByteArray(0), ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> { credit.creditFinalizedValue(id) }
        }
        assertEquals(0, endpoint.calls)
    }

    @Test
    fun `credit reads only positive bounded direct archive`() {
        val endpoint = Endpoint()
        val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
        val id = ByteArray(32) { 7 }
        assertContentEquals(byteArrayOf(3, 4, 5), credit.creditFinalizedValue(id))
        assertContentEquals(ByteArray(32) { 7 }, id)
        assertTrue(endpoint.direct)
        assertEquals(512, endpoint.capacity)
        assertEquals(1, endpoint.calls)
        for (status in listOf(-312, -311, 0, 513)) {
            endpoint.status = status
            assertEquals(status, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
                credit.creditFinalizedValue(id)
            }.status)
        }
        endpoint.missingCredit = true
        assertFailsWith<IllegalStateException> { credit.creditFinalizedValue(id) }
    }

    private class Endpoint : KagemushaTestnetValueCreditEndpointV1 {
        val contractWords = intArrayOf(1, 32, 512)
        var status = 3
        var calls = 0
        var direct = false
        var capacity = 0
        var missingContract = false
        var missingCredit = false

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract")
            return contractWords.copyOf()
        }

        override fun credit(operationId: ByteArray, output: ByteBuffer): Int {
            if (missingCredit) throw UnsatisfiedLinkError("missing credit")
            calls++
            operationId.fill(0)
            direct = output.isDirect
            capacity = output.capacity()
            output.put(byteArrayOf(3, 4, 5))
            return status
        }
    }
}
