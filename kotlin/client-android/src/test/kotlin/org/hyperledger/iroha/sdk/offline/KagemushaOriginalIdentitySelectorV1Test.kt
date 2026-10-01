// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Managed framing only. This endpoint fixture is no native reservation or enrollment authority. */
class KagemushaOriginalIdentitySelectorV1Test {
    @Test fun `selector read accepts no caller ID subject alias policy or extra response`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        val request = listOf(KagemushaCoreCoordinatorFrameV1.u32(11))
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
        assertContentEquals(request.single(), KagemushaCoreCoordinatorFrameV1.decodeRequest(method, encoded).single())
        for (extra in listOf(bytes(7), byteArrayOf(), "caller-alias".toByteArray())) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request + listOf(extra))
            }
        }
        for (response in listOf(emptyList(), listOf(ByteArray(32)), listOf(bytes(1).copyOf(31)), listOf(bytes(1), bytes(2)))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(method, encoded, response)
            }
        }
    }

    @Test fun `reading an original selector invokes no preparation and defensively owns returned bytes`() {
        val endpoint = Endpoint()
        val facade = facade(endpoint)
        val selected = facade.originalEnrollmentAttemptId()
        assertContentEquals(endpoint.id, selected)
        selected.fill(0)
        assertContentEquals(endpoint.id, facade.originalEnrollmentAttemptId())
        assertEquals(listOf(11, 11), endpoint.calls)
        assertEquals(0, endpoint.preparations)
    }

    @Test fun `prepare original identity uses the exact native selector and correlated original C`() {
        val endpoint = Endpoint()
        facade(endpoint).prepareOriginalIdentity()
        assertEquals(listOf(11, 1), endpoint.calls)
        assertContentEquals(endpoint.id, endpoint.selectedId)
        assertEquals(1, endpoint.preparations)
    }

    @Test fun `substituted reservation before preparation fails before any platform invocation`() {
        val endpoint = Endpoint(substitutePreparedId = true)
        assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareOriginalIdentity() }
        assertEquals(listOf(11, 1), endpoint.calls)
        assertEquals(1, endpoint.closes)
        assertEquals(1, endpoint.preparations)
    }

    private class Endpoint(private val substitutePreparedId: Boolean = false) : KagemushaCoreCoordinatorEndpointV1 {
        val id = bytes(1)
        val calls = ArrayList<Int>()
        var selectedId = byteArrayOf()
        var preparations = 0
        var closes = 0
        override fun contract() = intArrayOf(2, 25, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            check(method == 21)
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int
            calls.add(phase)
            return when (phase) {
                11 -> { assertEquals(1, fields.size); arrayOf(id.copyOf()) }
                1 -> {
                    preparations++
                    selectedId = fields[1].copyOf()
                    preparation(if (substitutePreparedId) bytes(14) else id)
                }
                else -> error("A selector read must not invoke key generation, attestation or financial operations")
            }
        }
    }

    companion object {
        private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
            KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture-only/reserved-identity-selector", endpoint))
        private fun preparation(id: ByteArray): Array<ByteArray> {
            val body = ByteArrayOutputStream().apply {
                write(byteArrayOf(1, 0, 1)); write(id)
                (2..13).forEach { write(bytes(it)) }
                write(le64(1)); write(le64(2)); write(le64(1000)); write(le64(121000))
            }.toByteArray()
            val c = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII) + le64(451) + body
            return arrayOf(le64(7), body + ByteArray(64) { 0x31 }, c, sha(c), byteArrayOf(5),
                KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(Charsets.UTF_8), byteArrayOf(1), bytes(0x20))
        }
        private fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
    }
}
