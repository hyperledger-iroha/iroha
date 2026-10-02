// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

class KagemushaAppOwnedHardwareFrameV1Test {
    @Test fun `initial publication phases accept only the original Bootstrap ticket and bounded commitments`() {
        for (phase in listOf(9, 10)) {
            val fields = listOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket)
            val request = KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalRequest(fields)
            val response = KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, publication)
            val decoded = KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalResponse(request, response)
            assertContentEquals(publication[8], decoded[8])
            decoded[8].fill(0)
            assertContentEquals(publication[8], KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalResponse(request, response)[8])
            for (invalid in listOf(fields.dropLast(1), fields + listOf(bytes(1)),
                listOf(fields[0], ByteArray(8)), listOf(fields[0], bytes(1)))) {
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalRequest(invalid) }
            }
            for (invalid in listOf(publication.dropLast(1), publication + listOf(bytes(1)))) {
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, invalid) }
            }
            for (index in publication.indices) {
                val invalid = publication.map(ByteArray::copyOf).toMutableList()
                invalid[index] = if (index == 0) le64(8) else ByteArray(32)
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, invalid) }
                invalid[index] = ByteArray(if (index == 0) 7 else 31) { 1 }
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, invalid) }
            }
        }
    }

    @Test fun `initial publication requests never enter generic monetary approval`() {
        for (phase in listOf(9, 10)) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL,
                    listOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket))
            }
        }
    }

    @Test fun `ordinary business input has no State authority and accepts exact positive u128 only`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL
        val fields = listOf(KagemushaCoreCoordinatorFrameV1.u32(15), KagemushaCoreCoordinatorFrameV1.u32(4), ByteArray(16) { 0xff.toByte() })
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields)
        val response = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(bytes(7)))
        assertContentEquals(bytes(7), KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, response)[0])
        for (bad in listOf(fields.dropLast(1), fields + listOf(bytes(8)),
            listOf(fields[0], fields[1], ByteArray(16)), listOf(fields[0], fields[1], ByteArray(15) { 1 }),
            listOf(fields[0], KagemushaCoreCoordinatorFrameV1.u32(1), fields[2]))) {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, bad) }
        }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, fields) }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(ByteArray(32))) }
    }

    private companion object {
        fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        val ticket = le64(7)
        fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        val publication = listOf(ticket) + (1..8).map(::bytes)
    }
    @Test fun completeStartChunksKeepMethod20DistinctAndRefuseBoundsAndSubstitution() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION
        val q = listOf(KagemushaCoreCoordinatorFrameV1.u32(15), ByteArray(8) { 1 }, KagemushaCoreCoordinatorFrameV1.u32(0))
        val r = listOf(q[2], ByteArray(65536) { 2 }, ByteArray(32) { 3 },
            KagemushaCoreCoordinatorFrameV1.u32(65537), ByteArray(32) { 4 }, ByteArray(32) { 5 })
        KagemushaAppOwnedHardwareFrameV1.requireRequest(method, q)
        KagemushaAppOwnedHardwareFrameV1.requireResponse(method, q, r)
        assertFailsWith<IllegalArgumentException> { KagemushaAppOwnedHardwareFrameV1.requireRequest(method,
            q.dropLast(1) + KagemushaCoreCoordinatorFrameV1.u32(4)) }
        assertFailsWith<IllegalArgumentException> { KagemushaAppOwnedHardwareFrameV1.requireResponse(method, q,
            r.toMutableList().also { it[0] = KagemushaCoreCoordinatorFrameV1.u32(1) }) }
        assertFailsWith<IllegalArgumentException> { KagemushaAppOwnedHardwareFrameV1.requireResponse(method, q,
            r.toMutableList().also { it[1] = ByteArray(65535) }) }
        assertFailsWith<IllegalArgumentException> { KagemushaAppOwnedHardwareFrameV1.requireResponse(method, q,
            r.toMutableList().also { it[3] = KagemushaCoreCoordinatorFrameV1.u32(262145) }) }
        assertFailsWith<IllegalArgumentException> { KagemushaAppOwnedHardwareFrameV1.requireRequest(
            KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL, q) }
        val last = listOf(KagemushaCoreCoordinatorFrameV1.u32(15), q[1], KagemushaCoreCoordinatorFrameV1.u32(1))
        KagemushaAppOwnedHardwareFrameV1.requireResponse(method, last,
            r.toMutableList().also { it[0] = last[2]; it[1] = byteArrayOf(2) })
    }

}
