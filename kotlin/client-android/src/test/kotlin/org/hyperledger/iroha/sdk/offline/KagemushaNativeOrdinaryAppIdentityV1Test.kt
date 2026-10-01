// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyEvidenceV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.KagemushaAndroidKeyAttestationArchiveV1
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Scripted framing/custody diagnostics. These fixtures are not a native owner or issuer admission. */
class KagemushaNativeOrdinaryAppIdentityV1Test {
    @Test fun `native generation fence precedes the only fresh platform call and returns original admission`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareOrdinaryIdentity(endpoint.id)
        assertContentEquals(endpoint.fields[2], prepared.originalChallengeSigningBytes())
        assertContentEquals(endpoint.fields[1], prepared.originalSignedPreparationBytes())
        prepared.originalSignedPreparationBytes().fill(0)
        assertContentEquals(endpoint.fields[1], prepared.originalSignedPreparationBytes())
        assertNull(prepared.recoverOriginalAdmission())
        var calls = 0
        val admission = prepared.performAndroidEnrollment { alias, challenge, policy, recoverOnly, guard ->
            calls++; assertEquals(1, endpoint.state); assertEquals(false, recoverOnly)
            assertEquals(endpoint.alias, alias); assertContentEquals(endpoint.fields[3], challenge)
            assertEquals(KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY, policy)
            guard(); endpoint.evidence()
        }
        assertEquals(1, calls); assertEquals(5, endpoint.state)
        assertContentEquals(endpoint.raw, admission.originalAttestationBytes())
        assertContentEquals(endpoint.signedRaw, admission.signedRawAdmissionTransport())
        assertContentEquals(endpoint.pending, admission.pendingNativeScopeDigest())
        assertContentEquals(endpoint.raw, prepared.performAndroidEnrollment { _, _, _, _, _ -> error("No second hardware invocation") }.originalAttestationBytes())
        assertEquals(1, calls)
    }

    @Test fun `signed preparation accessor checks the same native scope before exposure`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareOrdinaryIdentity(endpoint.id)
        endpoint.substituteScope = true
        assertFailsWith<IllegalStateException> { prepared.originalSignedPreparationBytes() }
        assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
    }

    @Test fun `unknown generation and attestation can only reopen the exact original read only`() {
        for (prior in listOf(1, 3)) {
            val endpoint = Endpoint().apply { state = prior }
            var calls = 0
            val admission = facade(endpoint).prepareOrdinaryIdentity(endpoint.id).performAndroidEnrollment { alias, challenge, _, recoverOnly, guard ->
                calls++; assertTrue(recoverOnly); assertEquals(endpoint.alias, alias)
                assertContentEquals(endpoint.fields[3], challenge); guard(); endpoint.evidence()
            }
            assertEquals(1, calls); assertEquals(0, endpoint.generationFences)
            assertContentEquals(endpoint.raw, admission.originalAttestationBytes())
        }
    }

    @Test fun `missing uncertain original freezes this capability instead of generating a replacement`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareOrdinaryIdentity(endpoint.id)
        var calls = 0
        assertFailsWith<IllegalStateException> {
            prepared.performAndroidEnrollment { _, _, _, _, _ -> calls++; error("Platform result unknown") }
        }
        assertEquals(1, endpoint.state)
        assertFailsWith<IllegalStateException> {
            prepared.performAndroidEnrollment { _, _, _, _, _ -> calls++; endpoint.evidence() }
        }
        assertFailsWith<IllegalStateException> { prepared.recoverOriginalAdmission() }
        assertEquals(1, calls)
        val reopened = facade(endpoint).prepareOrdinaryIdentity(endpoint.id)
        assertFailsWith<IllegalStateException> {
            reopened.performAndroidEnrollment { _, _, _, recoverOnly, _ -> assertTrue(recoverOnly); error("Original alias is missing") }
        }
        assertEquals(1, endpoint.generationFences)
    }

    @Test fun `raw retained recovery reads two exact chunks without any platform invocation`() {
        val endpoint = Endpoint(large = true).apply { state = 4; raw = archive }
        val admission = facade(endpoint).prepareOrdinaryIdentity(endpoint.id).performAndroidEnrollment { _, _, _, _, _ -> error("No platform recovery needed") }
        assertTrue(endpoint.raw.size > 65_536)
        assertEquals(listOf(0, 1), endpoint.chunkIndices)
        assertContentEquals(endpoint.raw, admission.originalAttestationBytes())
    }

    @Test fun `substituted native chunk hash or scope cannot expose a completed admission`() {
        val endpoint = Endpoint(large = true).apply { state = 4; raw = archive; corruptChunk = true }
        assertFailsWith<IllegalStateException> { facade(endpoint).prepareOrdinaryIdentity(endpoint.id).recoverOriginalAdmission() }
        assertEquals(1, endpoint.closes); assertEquals(4, endpoint.state)
        val replaced = Endpoint().apply { substituteScope = true }
        val prepared = facade(replaced).prepareOrdinaryIdentity(replaced.id)
        assertFailsWith<IllegalStateException> { prepared.originalChallengeSigningBytes() }
        assertEquals(0, replaced.generationFences); assertEquals(1, replaced.closes)
    }

    @Test fun `native C projection rejects unsigned body alias platform hash and legacy layout substitutions`() {
        for (field in listOf(1, 2, 3, 5)) {
            val endpoint = Endpoint(); endpoint.fields[field][0] = (endpoint.fields[field][0].toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareOrdinaryIdentity(endpoint.id) }
            assertEquals(0, endpoint.generationFences)
        }
        val legacy = Endpoint(); legacy.fields[1] = legacy.fields[1].copyOf(507)
        assertFailsWith<IllegalArgumentException> { facade(legacy).prepareOrdinaryIdentity(legacy.id) }
        val platform = Endpoint(); platform.fields[4] = byteArrayOf(4)
        assertFailsWith<IllegalArgumentException> { facade(platform).prepareOrdinaryIdentity(platform.id) }
    }

    @Test fun `chunk ABI preserves existing caps and refuses noncanonical split index and missing tail`() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        val ticket = le64(7)
        val request = listOf(KagemushaCoreCoordinatorFrameV1.u32(5), ticket, point, ByteArray(65_536) { 1 }, ByteArray(65_536) { 2 })
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
        assertContentEquals(encoded, KagemushaCoreCoordinatorFrameV1.encodeRequest(method, KagemushaCoreCoordinatorFrameV1.decodeRequest(method, encoded)))
        assertEquals(96 * 1024, KagemushaCoreCoordinatorFrameV1.MAXIMUM_FIELD_BYTES)
        assertEquals(128 * 1024, KagemushaCoreCoordinatorFrameV1.MAXIMUM_RESPONSE_BYTES)
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request.mapIndexed { index, value -> if (index == 3) value.copyOf(65_535) else value }) }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(10), ticket, KagemushaCoreCoordinatorFrameV1.u32(2))) }
        val read = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(10), ticket, KagemushaCoreCoordinatorFrameV1.u32(1)))
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, read, listOf(KagemushaCoreCoordinatorFrameV1.u32(1), byteArrayOf(1), bytes(9), KagemushaCoreCoordinatorFrameV1.u32(65_536)))
        }
    }

    private class Endpoint(large: Boolean = false) : KagemushaCoreCoordinatorEndpointV1 {
        val id = bytes(1)
        val fields = preparation(id).map(ByteArray::copyOf).toMutableList()
        val alias = fields[5].toString(Charsets.UTF_8)
        val chain = if (large) List(5) { der(16_384, it + 1) } else listOf(der(32, 1), der(32, 2))
        val archive = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(chain).transportBytes()
        val signedRaw = ByteArray(314) { 0x39 }
        val pending = bytes(0x40)
        var state = 0; var raw = byteArrayOf(); var closes = 0; var generationFences = 0
        var corruptChunk = false; var substituteScope = false
        val chunkIndices = ArrayList<Int>()
        fun evidence() = KagemushaAndroidHardwareAppKeyEvidenceV1(1, sha(point), point, chain)
        override fun contract() = intArrayOf(2, 25, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            check(method == 21)
            val phase = number(fields[0])
            return when (phase) {
                1 -> this.fields.toTypedArray()
                2 -> { check(state == 0); generationFences++; state = 1; arrayOf(byteArrayOf(1), byteArrayOf()) }
                3 -> { check(state == 1); assertContentEquals(this.fields[5], fields[2]); state = 2; arrayOf(sha(fields[2])) }
                4 -> { check(state == 2); state = 3; arrayOf(byteArrayOf(1), byteArrayOf(), byteArrayOf(), byteArrayOf()) }
                5 -> { check(state == 3); assertContentEquals(point, fields[2]); raw = fields[3] + fields[4]; state = 4; arrayOf(sha(raw), sha(point)) }
                6 -> { check(state >= 4); state = 5; arrayOf(pending, sha(signedRaw)) }
                7 -> arrayOf(byteArrayOf(state.toByte()), if (state >= 2) this.fields[5] else byteArrayOf(),
                    if (state >= 4) point else byteArrayOf(), if (state >= 4) sha(raw) else byteArrayOf(),
                    KagemushaCoreCoordinatorFrameV1.u32(if (state >= 4) raw.size else 0),
                    if (state == 5) signedRaw else byteArrayOf(), if (state == 5) pending else byteArrayOf())
                8 -> arrayOf(if (substituteScope) bytes(0x41) else this.fields[7], this.fields[3])
                9 -> { check(state == 0); emptyArray() }
                10 -> {
                    val index = number(fields[2]); chunkIndices.add(index)
                    val offset = index * 65_536
                    arrayOf(fields[2], raw.copyOfRange(offset, minOf(raw.size, offset + 65_536)),
                        if (corruptChunk) bytes(0x42) else sha(raw), KagemushaCoreCoordinatorFrameV1.u32(raw.size))
                }
                else -> error("Unexpected phase")
            }
        }
    }

    companion object {
        private val point = byteArrayOf(4) + ByteArray(64) { 7 }
        private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
            KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture-only/original-app-owner", endpoint))
        private fun preparation(id: ByteArray): List<ByteArray> {
            val cBody = ByteArrayOutputStream().apply {
                write(byteArrayOf(1, 0, 1)); write(id)
                (2..13).forEach { write(bytes(it)) }
                write(le64(1)); write(le64(2)); write(le64(1000)); write(le64(121000))
            }.toByteArray()
            val c = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII) + le64(451) + cBody
            return listOf(le64(7), cBody + ByteArray(64) { 0x31 }, c, sha(c), byteArrayOf(5),
                KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(Charsets.UTF_8), byteArrayOf(1), bytes(0x20))
        }
        private fun der(size: Int, marker: Int): ByteArray {
            val header = if (size < 130) byteArrayOf(0x30, (size - 2).toByte())
                else byteArrayOf(0x30, 0x82.toByte(), ((size - 4) ushr 8).toByte(), (size - 4).toByte())
            return header + ByteArray(size - header.size) { marker.toByte() }
        }
        private fun number(bytes: ByteArray) = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).int
        private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        private fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
    }
}
