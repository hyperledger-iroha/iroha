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

    private companion object {
        fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        val ticket = le64(7)
        fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        val publication = listOf(ticket) + (1..8).map(::bytes)
    }
}
