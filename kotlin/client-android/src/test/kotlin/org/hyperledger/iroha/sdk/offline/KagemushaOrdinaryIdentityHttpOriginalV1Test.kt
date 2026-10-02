// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.Base64
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Scripted actual-facade/codec diagnostics only; no native custody or issuer authority is created. */
class KagemushaOrdinaryIdentityHttpOriginalV1Test {
    @Test fun nativeReservationSelectsEntirePrepareCarrierAndRechecksAroundHttp() = runBlocking {
        val endpoint = Endpoint(); val held = facade(endpoint).reserveOriginalIdentity()
        assertContentEquals(endpoint.policy, held.originalPlayIntegrityPolicyBytes())
        var calls = 0
        val prepared = held.prepare { request ->
            calls++; request.requireCurrent()
            assertEquals("/v1/kagemusha/enrollment/ordinary/prepare", request.path)
            assertEquals(endpoint.reserved[7].toString(Charsets.US_ASCII), request.requestId)
            val body = objectFields(request.body())
            assertEquals(setOf("account_id", "client_nonce_hex", "release_id_hex", "profile_id_hex", "lane_id_hex", "financial_authority_commitment_hex"), body.keys)
            assertEquals("fixture-account", body["account_id"])
            assertEquals(hex(endpoint.reserved[2]), body["client_nonce_hex"])
            assertEquals(0, endpoint.intakes)
            request.body().fill(0); assertEquals(body, objectFields(request.body()))
            endpoint.prepareReply()
        }
        assertEquals(1, calls); assertEquals(1, endpoint.intakes)
        assertContentEquals(endpoint.prepared[1], prepared.originalSignedPreparationBytes())
        assertTrue(endpoint.calls.count { it == 12 } >= 4)
        assertFalse(endpoint.calls.contains(1)); assertEquals(0, endpoint.rawIntakes)
    }

    @Test fun substitutedPrepareResponseCannotReachNativeIntake() = runBlocking {
        for (change in listOf("nonce", "digest", "unknown", "duplicate", "bad-utf8")) {
            val endpoint = Endpoint()
            assertFails { facade(endpoint).prepareOriginalIdentity { request ->
                request.requireCurrent(); endpoint.prepareReply(change)
            } }
            assertEquals(0, endpoint.intakes)
        }
    }

    @Test fun nativeReservationReplacementDuringHttpStopsBeforePreparation() = runBlocking {
        val endpoint = Endpoint()
        assertFailsWith<IllegalStateException> {
            facade(endpoint).prepareOriginalIdentity { request ->
                request.requireCurrent(); endpoint.changed = true; endpoint.prepareReply()
            }
        }
        assertEquals(0, endpoint.intakes); assertEquals(1, endpoint.closes)
    }

    @Test fun rawOriginalIsTransportedBeforeExplicit314IntakeAndAdmittedRecoveryDoesNotHttp() = runBlocking {
        val endpoint = Endpoint().apply { state = 4 }
        val prepared = facade(endpoint).prepareOriginalIdentity { endpoint.prepareReply() }
        var calls = 0
        val admitted = prepared.admitOriginalAttestation { request ->
            calls++; request.requireCurrent()
            assertEquals("/v1/kagemusha/enrollment/ordinary/raw-attestation", request.path)
            val body = objectFields(request.body())
            assertEquals(hex(sha(endpoint.prepared[2])), body["operation_id"])
            assertEquals(base64(endpoint.raw), body["raw_attestation_base64"])
            assertEquals(base64(endpoint.point), body["attested_public_key_sec1_base64"])
            assertEquals(0, endpoint.rawIntakes)
            endpoint.rawReply()
        }
        assertEquals(1, calls); assertEquals(1, endpoint.rawIntakes)
        assertContentEquals(endpoint.signedRaw, admitted.signedRawAdmissionTransport())
        val recovered = prepared.admitOriginalAttestation { error("Already native-admitted original must not call issuer") }
        assertContentEquals(endpoint.signedRaw, recovered.signedRawAdmissionTransport())
        assertEquals(1, endpoint.rawIntakes)
    }

    @Test fun missingOrSubstitutedRawOriginalNeverReachesRawIntake() = runBlocking {
        val missing = Endpoint().apply { state = 3 }
        val prepared = facade(missing).prepareOriginalIdentity { missing.prepareReply() }
        assertFails { prepared.admitOriginalAttestation { error("No HTTP without retained raw") } }
        assertEquals(0, missing.rawIntakes)
        val bad = Endpoint().apply { state = 4 }
        val badPrepared = facade(bad).prepareOriginalIdentity { bad.prepareReply() }
        assertFails { badPrepared.admitOriginalAttestation { bad.rawReply(corruptHash = true) } }
        assertEquals(0, bad.rawIntakes); assertEquals(4, bad.state)
    }

    @Test fun pureHttpCodecRejectsMalformedReservationWidthsAndWrongRawAdmissionBoundary() {
        val e = Endpoint()
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.preparationBody(e.reserved.dropLast(1)) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(ByteArray(32)) }
        for (width in listOf(0, 313, 315)) {
            val raw = ByteArray(width) { 7 }
            assertFails { KagemushaOrdinaryIdentityHttpCodecV1.rawAdmissionResponse(json(linkedMapOf(
                "raw_admission_base64" to base64(raw), "raw_admission_sha256_hex" to hex(sha(raw))))) }
        }
    }

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val point = byteArrayOf(4) + ByteArray(64) { 7 }
        val raw = KagemushaPlatformAttestationOriginalV1.android(listOf(
            byteArrayOf(0x30, 2, 1, 1), byteArrayOf(0x30, 2, 1, 2))).canonicalBytes()
        val signedRaw = ByteArray(314) { 0x39 }; val pending = bytes(0x40)
        val policy = "inert pinned policy".toByteArray()
        val nonce = bytes(2)
        val reserved = listOf(le64(6), "fixture-account".toByteArray(), nonce, bytes(7), bytes(8), bytes(6), bytes(12), uuid(nonce).toByteArray())
        val prepared = preparation(nonce)
        val calls = ArrayList<Int>()
        var changed = false; var intakes = 0; var rawIntakes = 0; var closes = 0; var state = 0
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            check(method == 21)
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int; calls.add(phase)
            return when (phase) {
                12 -> reserved.mapIndexed { i, b -> if (changed && i == 3) bytes(99) else b.copyOf() }.toTypedArray()
                13 -> { assertContentEquals(reserved[0], fields[1]); assertContentEquals(prepared[1], fields[2]); intakes++; prepared.toTypedArray() }
                14 -> arrayOf(policy.copyOf())
                6 -> { assertContentEquals(signedRaw, fields[2]); rawIntakes++; state = 5; arrayOf(pending, sha(signedRaw)) }
                7 -> arrayOf(byteArrayOf(state.toByte()), if (state >= 2) prepared[5] else byteArrayOf(),
                    if (state >= 4) point else byteArrayOf(), if (state >= 4) sha(raw) else byteArrayOf(),
                    KagemushaCoreCoordinatorFrameV1.u32(if (state >= 4) raw.size else 0),
                    if (state == 5) signedRaw else byteArrayOf(), if (state == 5) pending else byteArrayOf())
                8 -> arrayOf(prepared[7], prepared[3])
                10 -> { assertEquals(0, ByteBuffer.wrap(fields[2]).order(ByteOrder.LITTLE_ENDIAN).int)
                    arrayOf(fields[2], raw, sha(raw), KagemushaCoreCoordinatorFrameV1.u32(raw.size)) }
                else -> error("This HTTP fixture cannot invoke generation, attestation, approval or finance")
            }
        }
        fun prepareReply(change: String = ""): ByteArray {
            val offered = if (change == "nonce") preparation(bytes(14)) else prepared
            val fields = linkedMapOf<String, Any?>("operation_id" to hex(sha(offered[2])),
                "signed_preparation_base64" to base64(offered[1]),
                "attestation_challenge_base64" to base64(if (change == "digest") bytes(77) else sha(offered[2])),
                "expires_at_ms" to 121000L)
            if (change == "unknown") fields["authority_verdict"] = true
            val result = json(fields)
            return when (change) {
                "duplicate" -> (result.toString(Charsets.UTF_8).dropLast(1) + ",\"expires_at_ms\":121000}").toByteArray()
                "bad-utf8" -> byteArrayOf(0xc0.toByte(), 0x80.toByte())
                else -> result
            }
        }
        fun rawReply(corruptHash: Boolean = false) = json(linkedMapOf(
            "raw_admission_base64" to base64(signedRaw), "raw_admission_sha256_hex" to hex(if (corruptHash) bytes(77) else sha(signedRaw))))
    }
    companion object {
        private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
            KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture-only/native-carrier", endpoint))
        private fun preparation(nonce: ByteArray): List<ByteArray> {
            val body = byteArrayOf(1, 0, 1) + bytes(1) + nonce + (3..13).flatMap { bytes(it).toList() }.toByteArray() +
                le64(1) + le64(2) + le64(1000) + le64(121000)
            val signed = body + ByteArray(64) { 0x31 }
            val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signed).canonicalSigningBytes()
            return listOf(le64(7), signed, c, sha(c), byteArrayOf(5),
                KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(), byteArrayOf(1), bytes(0x20))
        }
        private fun uuid(nonce: ByteArray): String {
            val b = nonce.copyOfRange(0, 16); b[6] = 0x42; b[8] = 0x82.toByte()
            return UUID(ByteBuffer.wrap(b).long, ByteBuffer.wrap(b, 8, 8).long).toString()
        }
        private fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        private fun le64(n: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(n).array()
        private fun sha(b: ByteArray) = MessageDigest.getInstance("SHA-256").digest(b)
        private fun hex(b: ByteArray) = b.joinToString("") { "%02x".format(it.toInt() and 255) }
        private fun base64(b: ByteArray) = Base64.getEncoder().encodeToString(b)
        private fun json(fields: Map<String, Any?>) = JsonEncoder.encode(fields).toByteArray(Charsets.UTF_8)
        @Suppress("UNCHECKED_CAST") private fun objectFields(raw: ByteArray) = JsonParser.parse(raw.toString(Charsets.UTF_8)) as Map<String, Any?>
    }
}
