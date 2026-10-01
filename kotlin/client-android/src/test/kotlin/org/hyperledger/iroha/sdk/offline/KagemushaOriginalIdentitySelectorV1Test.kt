// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.UUID
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Managed framing only. This endpoint fixture is no native reservation or enrollment authority. */
class KagemushaOriginalIdentitySelectorV1Test {
    @Test fun `selector and reservation requests accept no caller subject while implicit preparation is retired`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        for (phase in listOf(11, 12)) {
            val request = listOf(KagemushaCoreCoordinatorFrameV1.u32(phase))
            val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
            assertContentEquals(request.single(), KagemushaCoreCoordinatorFrameV1.decodeRequest(method, encoded).single())
            for (extra in listOf(bytes(7), byteArrayOf(), "caller-alias".toByteArray())) {
                assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request + listOf(extra)) }
            }
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(1), bytes(1)))
        }
    }

    @Test fun `reservation carrier validates exact cardinality nonzero selectors canonical UUID and UTF8`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(12)))
        val valid = carrier()
        val invalid = listOf(emptyList(), valid.dropLast(1), valid + listOf(bytes(1)),
            valid.mapIndexed { i, b -> if (i == 2) ByteArray(32) else b },
            valid.mapIndexed { i, b -> if (i == 1) byteArrayOf(0xc0.toByte(), 0x80.toByte()) else b },
            valid.mapIndexed { i, b -> if (i == 7) UUID.randomUUID().toString().toByteArray() else b })
        for (fields in invalid) assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, fields)
        }
        KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, valid)
    }

    @Test fun `reading an original ID invokes no reservation and owns returned bytes`() {
        val endpoint = Endpoint(); val facade = facade(endpoint)
        val selected = facade.originalEnrollmentAttemptId()
        assertContentEquals(endpoint.id, selected); selected.fill(0)
        assertContentEquals(endpoint.id, facade.originalEnrollmentAttemptId())
        assertEquals(listOf(11, 11), endpoint.calls); assertEquals(0, endpoint.preparations)
    }

    @Test fun `native reservation exposes copied carrier without implicitly preparing or generating`() {
        val endpoint = Endpoint(); val held = facade(endpoint).reserveOriginalIdentity()
        assertEquals("fixture-account", held.accountId())
        assertContentEquals(endpoint.reserved[2], held.clientNonce()); held.clientNonce().fill(0)
        assertContentEquals(endpoint.reserved[2], held.clientNonce())
        assertContentEquals(endpoint.reserved[3], held.releaseId())
        assertContentEquals(endpoint.reserved[4], held.hardwareProfileId())
        assertContentEquals(endpoint.reserved[5], held.laneId())
        assertContentEquals(endpoint.reserved[6], held.financialAuthorityCommitment())
        assertEquals(endpoint.reserved[7].toString(Charsets.UTF_8), held.requestId())
        assertEquals(0, endpoint.preparations)
        assertEquals(setOf(12), endpoint.calls.toSet())
    }

    @Test fun `explicit preparation intake retains exact reservation ticket and original515`() {
        val endpoint = Endpoint(); val held = facade(endpoint).reserveOriginalIdentity()
        val offered = endpoint.prepared[1].copyOf()
        val prepared = held.acceptOriginalSignedPreparation(offered)
        offered.fill(0)
        assertContentEquals(endpoint.prepared[1], prepared.originalSignedPreparationBytes())
        assertContentEquals(endpoint.reserved[0], endpoint.selectedTicket)
        assertContentEquals(endpoint.prepared[1], endpoint.selectedPreparation)
        held.acceptOriginalSignedPreparation(endpoint.prepared[1])
        assertEquals(2, endpoint.preparations)
        assertFailsWith<IllegalStateException> {
            held.acceptOriginalSignedPreparation(endpoint.prepared[1].copyOf().apply { this[514] = 0x32 })
        }
        assertEquals(2, endpoint.preparations); assertEquals(1, endpoint.closes)
    }

    @Test fun `validly shaped C cannot substitute the already retained native reservation selectors`() {
        val endpoint = Endpoint(substitutePreparedNonce = true)
        val held = facade(endpoint).reserveOriginalIdentity()
        assertFailsWith<IllegalStateException> { held.acceptOriginalSignedPreparation(endpoint.prepared[1]) }
        assertEquals(1, endpoint.preparations); assertEquals(1, endpoint.closes)
    }

    @Test fun `policy original is guarded readonly data and reservation substitution revokes it`() {
        val endpoint = Endpoint(); val held = facade(endpoint).reserveOriginalIdentity()
        val original = held.originalPlayIntegrityPolicyBytes()
        assertContentEquals(endpoint.policyOriginal, original); original.fill(0)
        assertContentEquals(endpoint.policyOriginal, held.originalPlayIntegrityPolicyBytes())
        assertEquals(0, endpoint.preparations)
        endpoint.substituteReservation = true
        assertFailsWith<IllegalStateException> { held.originalPlayIntegrityPolicyBytes() }
        assertEquals(1, endpoint.closes)
    }

    @Test fun `explicit raw intake requires original314 and policy response remains bounded`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(6), le64(7)))
        }
        for (size in listOf(0, 313, 315)) assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(6), le64(7), ByteArray(size)))
        }
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(14), le64(6)))
        KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(byteArrayOf()))
        KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(ByteArray(16384)))
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(ByteArray(16385))) }
    }

    private class Endpoint(substitutePreparedNonce: Boolean = false) : KagemushaCoreCoordinatorEndpointV1 {
        val id = bytes(1)
        val reserved = carrier()
        val prepared = preparation(if (substitutePreparedNonce) bytes(14) else bytes(2))
        val policyOriginal = "inert scripted policy original".toByteArray()
        val calls = ArrayList<Int>()
        var selectedTicket = byteArrayOf(); var selectedPreparation = byteArrayOf()
        var preparations = 0; var closes = 0; var substituteReservation = false
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            check(method == 21)
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int; calls.add(phase)
            return when (phase) {
                11 -> arrayOf(id.copyOf())
                12 -> reserved.mapIndexed { i, b -> if (substituteReservation && i == 3) bytes(15) else b.copyOf() }.toTypedArray()
                13 -> {
                    preparations++; selectedTicket = fields[1].copyOf(); selectedPreparation = fields[2].copyOf()
                    assertContentEquals(reserved[0], fields[1]); assertContentEquals(prepared[1], fields[2]); prepared
                }
                14 -> arrayOf(policyOriginal.copyOf())
                8 -> arrayOf(prepared[7], prepared[3])
                else -> error("A carrier read must not invoke generation, attestation or financial operations")
            }
        }
    }

    companion object {
        private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
            KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture-only/reserved-identity-selector", endpoint))
        private fun carrier(): List<ByteArray> {
            val nonce = bytes(2)
            val uuid = nonce.copyOf(16).apply {
                this[6] = ((this[6].toInt() and 15) or 0x40).toByte()
                this[8] = ((this[8].toInt() and 63) or 0x80).toByte()
            }
            return listOf(le64(6), "fixture-account".toByteArray(), nonce, bytes(7), bytes(8), bytes(6), bytes(12),
                UUID(ByteBuffer.wrap(uuid).long, ByteBuffer.wrap(uuid, 8, 8).long).toString().toByteArray())
        }
        private fun preparation(nonce: ByteArray): Array<ByteArray> {
            val body = ByteArrayOutputStream().apply {
                write(byteArrayOf(1, 0, 1)); write(bytes(1)); write(nonce)
                (3..13).forEach { write(bytes(it)) }
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
