// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import java.util.Base64
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

/** Scripted original-owner projections exercise control flow, never FI or native authority. */
class KagemushaNativeOrdinaryRetailEnrollmentV1Test {
    @Test fun nativeFencePrecedesOneRawWalletSignatureAndCompletedRetriesRecoverIt() {
        val endpoint = Endpoint()
        val fields = endpoint.fields()
        val holder = owner(endpoint, fields)
        fields.forEach { it.fill(0) }
        val signing = holder.accountSigningBytes()
        assertContentEquals(endpoint.message, signing)
        signing.fill(0)
        assertContentEquals(endpoint.message, holder.accountSigningBytes())
        assertNull(holder.recoverOriginalAccountSignature())
        var calls = 0
        val signature = holder.signOriginalAccount { original ->
            calls++
            assertEquals(1, endpoint.state)
            assertContentEquals(endpoint.message, original)
            original.fill(0)
            endpoint.expectedSignature.copyOf()
        }
        assertEquals(1, calls)
        assertEquals(1, endpoint.retains)
        assertContentEquals(endpoint.expectedSignature, signature)
        signature.fill(0)
        assertContentEquals(endpoint.expectedSignature, holder.recoverOriginalAccountSignature())
        assertContentEquals(endpoint.expectedSignature, holder.signOriginalAccount { error("Must recover the retained original") })
        assertEquals(1, calls)
        assertEquals(1, endpoint.retains)
    }

    @Test fun uncertainWalletOutcomeRetainsFenceAndFreshRecoveryCannotSignAgain() {
        val endpoint = Endpoint()
        val holder = owner(endpoint)
        var calls = 0
        assertFailsWith<KagemushaNativeRetailSigningUnknownOutcomeExceptionV1> {
            holder.signOriginalAccount { calls++; error("Wallet return lost") }
        }
        assertEquals(1, endpoint.state)
        assertFailsWith<IllegalStateException> { holder.signOriginalAccount { calls++; endpoint.expectedSignature } }
        assertEquals(1, calls)
        val recovered = owner(endpoint)
        assertFailsWith<KagemushaNativeRetailSigningUnknownOutcomeExceptionV1> { recovered.recoverOriginalAccountSignature() }
        assertFailsWith<KagemushaNativeRetailSigningUnknownOutcomeExceptionV1> {
            recovered.signOriginalAccount { calls++; endpoint.expectedSignature }
        }
        assertEquals(1, calls)
        assertEquals(0, endpoint.retains)
    }

    @Test fun retainedSignatureWithLostNativeReturnClosesOldBridgeAndFreshOwnerRecoversOnlyOriginal() {
        val endpoint = Endpoint().apply { loseRetainReturn = true }
        val holder = owner(endpoint)
        assertFailsWith<KagemushaNativeRetailSigningUnknownOutcomeExceptionV1> {
            holder.signOriginalAccount { endpoint.expectedSignature.copyOf() }
        }
        assertEquals(2, endpoint.state)
        assertEquals(1, endpoint.closes)
        assertEquals(1, endpoint.retains)
        assertFailsWith<IllegalStateException> { holder.accountSigningBytes() }
        endpoint.loseRetainReturn = false
        val recovered = owner(endpoint)
        assertContentEquals(endpoint.expectedSignature, recovered.signOriginalAccount { error("Must not sign twice") })
        assertEquals(1, endpoint.retains)
    }

    @Test fun malformedSignatureCannotReachNativeRetentionOrFiTransport() {
        for (size in listOf(0, 63, 65)) {
            val endpoint = Endpoint()
            val holder = owner(endpoint)
            assertFailsWith<KagemushaNativeRetailSigningUnknownOutcomeExceptionV1> {
                holder.signOriginalAccount { ByteArray(size) }
            }
            assertEquals(1, endpoint.state)
            assertEquals(0, endpoint.retains)
            assertFailsWith<IllegalStateException> { holder.finishRequestOriginal() }
        }
    }

    @Test fun exactProtectedFiReplyIsAdmittedBySameTicketBeforeCertificateExposure() = runBlocking {
        val endpoint = Endpoint()
        val holder = owner(endpoint)
        assertFailsWith<IllegalStateException> { holder.originalRetailCertificate() }
        holder.signOriginalAccount { endpoint.expectedSignature.copyOf() }
        val request = holder.finishRequestOriginal()
        assertEquals("/v1/offline/enrollment/ordinary/finish", request.path)
        assertEquals(KagemushaOrdinaryIdentityHttpCodecV1.retailRequestId(endpoint.id, "finish"), request.requestId)
        val body = KagemushaOrdinaryIdentityHttpCodecV1.retailFinishBody(endpoint.id, endpoint.expectedSignature)
        assertContentEquals(body, request.body())
        request.body().fill(0)
        assertContentEquals(body, request.body())
        var exchanges = 0
        val admitted = holder.completeOriginalEnrollment {
            exchanges++
            assertContentEquals(body, it.body())
            assertEquals(2, endpoint.state)
            endpoint.finishReply()
        }
        assertEquals(1, exchanges)
        assertEquals(1, endpoint.admissions)
        assertEquals(3, endpoint.state)
        assertContentEquals(endpoint.enrollmentId, admitted)
        admitted.fill(0)
        val certificate = holder.originalRetailCertificate()
        assertContentEquals(endpoint.certificate, certificate)
        certificate.fill(0)
        assertContentEquals(endpoint.certificate, holder.originalRetailCertificate())
    }

    @Test fun substitutedPublicOrNativeFiIdentityCannotExposeCompletion() = runBlocking {
        for (mutation in listOf("challenge", "id", "extra", "native-id", "native-scope")) {
            val endpoint = Endpoint()
            val holder = owner(endpoint)
            holder.signOriginalAccount { endpoint.expectedSignature.copyOf() }
            if (mutation == "native-id") endpoint.substituteId = true
            if (mutation == "native-scope") endpoint.substituteScope = true
            if (mutation in listOf("challenge", "extra")) {
                assertFailsWith<IllegalArgumentException> {
                    holder.completeOriginalEnrollment { endpoint.finishReply(mutation) }
                }
                assertEquals(0, endpoint.admissions)
            } else {
                assertFailsWith<IllegalStateException> {
                    holder.completeOriginalEnrollment { endpoint.finishReply(mutation) }
                }
                assertEquals(1, endpoint.closes)
            }
        }
    }

    @Test fun transportOwnerChangeStopsBeforeNativeAdmissionAndRetainedSubstitutionClosesBridge() = runBlocking {
        val endpoint = Endpoint()
        var live = true
        val holder = owner(endpoint, guard = { check(live) })
        holder.signOriginalAccount { endpoint.expectedSignature.copyOf() }
        assertFailsWith<IllegalStateException> {
            holder.completeOriginalEnrollment { live = false; endpoint.finishReply() }
        }
        assertEquals(0, endpoint.admissions)
        val changed = Endpoint()
        val retained = owner(changed)
        retained.signOriginalAccount { changed.expectedSignature.copyOf() }
        changed.signature[0] = (changed.signature[0].toInt() xor 1).toByte()
        assertFailsWith<IllegalStateException> { retained.recoverOriginalAccountSignature() }
        assertEquals(1, changed.closes)
    }

    @Test fun cancelOnlyUninvokedCeremonyAndRejectForeignNativeScope() {
        val endpoint = Endpoint()
        val holder = owner(endpoint)
        holder.cancel()
        assertEquals(1, endpoint.cancels)
        assertFailsWith<IllegalStateException> { holder.accountSigningBytes() }
        val invoked = Endpoint().apply { state = 1 }
        assertFailsWith<IllegalStateException> { owner(invoked).cancel() }
        assertEquals(0, invoked.cancels)
        val substituted = Endpoint()
        val fields = substituted.fields()
        fields[3][0] = (fields[3][0].toInt() xor 1).toByte()
        assertFailsWith<IllegalStateException> { owner(substituted, fields) }
        assertEquals(1, substituted.closes)
    }

    private fun owner(endpoint: Endpoint, fields: List<ByteArray> = endpoint.fields(), guard: () -> Unit = {}) =
        KagemushaNativeOrdinaryRetailEnrollmentV1.fromNative(
            KagemushaCoreCoordinatorBridgeV1.openEndpoint("/test/retail-original", endpoint),
            endpoint.id, fields, endpoint.scope, endpoint.credentialDigest, guard)

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val id = ByteArray(32) { 1 }
        val ticket = ByteArray(8) { 2 }
        val message = ByteArray(32) { 3 }
        val scope = ByteArray(32) { 4 }
        val credentialDigest = ByteArray(32) { 5 }
        val expectedSignature = ByteArray(64) { 6 }
        val certificate = byteArrayOf(7, 8, 9)
        val enrollmentId = sha(certificate)
        var state = 0
        var signature = byteArrayOf()
        var retains = 0
        var admissions = 0
        var closes = 0
        var cancels = 0
        var loseRetainReturn = false
        var substituteId = false
        var substituteScope = false
        fun fields() = listOf(ticket, byteArrayOf(10, 11), message, scope, credentialDigest).map(ByteArray::copyOf)
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            assertEquals(20, method)
            assertContentEquals(ticket, fields[1])
            return when (fields[0][0].toInt()) {
                10 -> if (state in 2..3) arrayOf(byteArrayOf(2), signature.copyOf())
                    else { check(state == 0); state = 1; arrayOf(byteArrayOf(1), byteArrayOf()) }
                11 -> { check(state == 1); retains++; signature = fields[2].copyOf(); state = 2
                    if (loseRetainReturn) null else arrayOf(sha(signature)) }
                12 -> { check(state == 2); assertContentEquals(certificate, fields[2]); admissions++; state = 3
                    arrayOf(if (substituteId) ByteArray(32) { 12 } else enrollmentId.copyOf(),
                        if (substituteScope) ByteArray(32) { 13 } else scope.copyOf()) }
                13 -> arrayOf(byteArrayOf(state.toByte()), signature.copyOf(), if (state == 3) certificate.copyOf() else byteArrayOf())
                14 -> { check(state == 0); cancels++; emptyArray() }
                else -> error("Unexpected retail phase")
            }
        }
        fun finishReply(mutation: String? = null): ByteArray {
            val challenge = if (mutation == "challenge") ByteArray(32) { 14 } else id
            val identity = if (mutation == "id") ByteArray(32) { 15 } else enrollmentId
            val extra = if (mutation == "extra") ",\"unexpected\":1" else ""
            return ("{\"challenge_id\":\"${hex(challenge)}\",\"enrollment_id_hex\":\"${hex(identity)}\"," +
                "\"canonical_certificate_base64\":\"${Base64.getEncoder().encodeToString(certificate)}\"$extra}").toByteArray(Charsets.UTF_8)
        }
    }
    private companion object {
        fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
        fun hex(raw: ByteArray) = raw.joinToString("") { "%02x".format(it.toInt() and 255) }
    }
}
