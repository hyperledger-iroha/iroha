// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaCoreCoordinatorBridgeV1Test {
    @Test
    fun `native transport retains the caller identity even if JNI mutates its inputs`() {
        val endpoint = Endpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/store", endpoint)
        val id = ByteArray(32) { 7 }
        val fields = listOf(KagemushaCoreCoordinatorFrameV1.u32(22), id, byteArrayOf(1))
        assertContentEquals(id, bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID, fields).single())
        endpoint.mutateRequest = true
        assertFailsWith<IllegalArgumentException> { bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID, fields) }
        assertContentEquals(ByteArray(32) { 7 }, id)
    }

    @Test
    fun `missing backend or drifted ABI never opens the coordinator`() {
        val mismatch = Endpoint().apply { contractWords[0] = 1 }
        assertFailsWith<IllegalStateException> { KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/store", mismatch) }
        assertEquals(0, mismatch.openCalls)
        val missing = Endpoint().apply { returnedHandle = 0 }
        assertFailsWith<IllegalStateException> { KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/store", missing) }
    }

    @Test
    fun `invalid storage paths and requests fail before native calls`() {
        val endpoint = Endpoint()
        listOf("", " ", "nul\u0000path", "x".repeat(4097), "bad\ud800").forEach {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorBridgeV1.openEndpoint(it, endpoint) }
        }
        assertEquals(0, endpoint.openCalls)
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/🔒", endpoint)
        assertFailsWith<IllegalArgumentException> { bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID, emptyList()) }
        assertEquals(0, endpoint.invokeCalls)
        endpoint.missingResponse = true
        assertFailsWith<IllegalStateException> {
            bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(22), ByteArray(32) { 7 }, byteArrayOf(1)))
        }
    }

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val contractWords = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff)
        var openCalls = 0
        var invokeCalls = 0
        var returnedHandle = -1L // An opaque u64 handle retains every bit across JNI's signed long.
        var mutateRequest = false
        var missingResponse = false
        override fun contract() = contractWords.copyOf()
        override fun open(storagePath: String): Long { openCalls++; return returnedHandle }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            invokeCalls++
            assertEquals(returnedHandle, handle)
            assertEquals(1, method)
            if (missingResponse) return null
            if (mutateRequest) fields[1].fill(8)
            return arrayOf(fields[1])
        }
    }
}
