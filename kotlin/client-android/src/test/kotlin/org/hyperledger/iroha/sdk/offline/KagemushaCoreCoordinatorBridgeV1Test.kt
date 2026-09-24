// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import org.hyperledger.iroha.sdk.offline.probe.KagemushaTestnetStateProofObservationEndpointV1
import org.hyperledger.iroha.sdk.offline.probe.KagemushaTestnetStateProofObservationV1
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
    fun `retained outgoing State archives are bounded detached and operation bound`() {
        val endpoint = Endpoint()
        val core = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/store", endpoint)
        val operation = ByteArray(32) { 9 }
        val pair = core.exportOutgoingStateProof(operation)
        assertContentEquals(operation, pair.operationId())
        assertContentEquals(byteArrayOf(1, 2), pair.publicInputsArchive())
        assertContentEquals(byteArrayOf(3, 4), pair.pairedProofArchive())
        pair.publicInputsArchive().fill(0)
        pair.pairedProofArchive().fill(0)
        assertContentEquals(byteArrayOf(1, 2), pair.publicInputsArchive())
        assertContentEquals(byteArrayOf(3, 4), pair.pairedProofArchive())
        val observer = KagemushaTestnetStateProofObservationV1.openEndpoint(
            object : KagemushaTestnetStateProofObservationEndpointV1 {
                override fun contract(): IntArray = intArrayOf(1, 4096, 6528, 256)

                override fun observe(
                    publicInputsArchive: ByteArray,
                    pairedProofArchive: ByteArray,
                    output: ByteBuffer,
                ): Int {
                    assertContentEquals(byteArrayOf(1, 2), publicInputsArchive)
                    assertContentEquals(byteArrayOf(3, 4), pairedProofArchive)
                    output.put(0x42.toByte())
                    return 1
                }
            },
        )
        assertContentEquals(byteArrayOf(0x42), pair.observeWith(observer))
        endpoint.substituteExportOperation = true
        assertFailsWith<IllegalArgumentException> { core.exportOutgoingStateProof(operation) }
        endpoint.substituteExportOperation = false
        endpoint.oversizeExportProof = true
        assertFailsWith<IllegalArgumentException> { core.exportOutgoingStateProof(operation) }
        core.close()
        assertFailsWith<IllegalStateException> { core.exportOutgoingStateProof(operation) }
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

    @Test
    fun `close revokes local handle and cannot dispatch or close twice`() {
        val endpoint = Endpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/store", endpoint)
        bridge.close()
        bridge.close()
        assertEquals(1, endpoint.closeCalls)
        assertFailsWith<IllegalStateException> {
            bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(22), ByteArray(32) { 7 }, byteArrayOf(1)))
        }
        assertEquals(0, endpoint.invokeCalls)
    }

    @Test
    fun `failed native teardown still revokes local handle`() {
        val endpoint = Endpoint().apply { closeStatus = -312 }
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/durable/store", endpoint)
        assertFailsWith<IllegalStateException> { bridge.close() }
        assertFailsWith<IllegalStateException> {
            bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(22), ByteArray(32) { 7 }, byteArrayOf(1)))
        }
        assertEquals(1, endpoint.closeCalls)
        assertEquals(0, endpoint.invokeCalls)
    }

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val contractWords = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 14)
        var openCalls = 0
        var invokeCalls = 0
        var closeCalls = 0
        var closeStatus = 0
        var returnedHandle = -1L // An opaque u64 handle retains every bit across JNI's signed long.
        var mutateRequest = false
        var missingResponse = false
        var substituteExportOperation = false
        var oversizeExportProof = false
        override fun contract() = contractWords.copyOf()
        override fun open(storagePath: String): Long { openCalls++; return returnedHandle }
        override fun close(handle: Long): Int { closeCalls++; assertEquals(returnedHandle, handle); return closeStatus }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            invokeCalls++
            assertEquals(returnedHandle, handle)
            if (method == KagemushaCoreCoordinatorMethodV1.EXPORT_OUTGOING_STATE_PROOF.code) {
                if (missingResponse) return null
                return arrayOf(
                    if (substituteExportOperation) ByteArray(32) { 8 } else fields[0],
                    byteArrayOf(1, 2),
                    if (oversizeExportProof) ByteArray(6_529) else byteArrayOf(3, 4),
                )
            }
            assertEquals(1, method)
            if (missingResponse) return null
            if (mutateRequest) fields[1].fill(8)
            return arrayOf(fields[1])
        }
    }
}
