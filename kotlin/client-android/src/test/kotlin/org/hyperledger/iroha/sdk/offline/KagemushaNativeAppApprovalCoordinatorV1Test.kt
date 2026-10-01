// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

/** Scripted ABI fixtures test JVM correlation/once-only control flow, not installed native authority. */
class KagemushaNativeAppApprovalCoordinatorV1Test {
    @Test fun `native fence precedes exactly one signing call and retries return retained receipt`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        var signs = 0
        val receipt = prepared.performPlatformSigning { alias, challenge, point, key, message, policy, guard ->
            signs++; assertEquals(1, endpoint.state); assertEquals(endpoint.fields[3].toString(Charsets.UTF_8), alias)
            assertContentEquals(endpoint.fields[4], challenge); assertContentEquals(endpoint.fields[5], point)
            assertContentEquals(endpoint.fields[6], key); assertContentEquals(endpoint.fields[1], message)
            assertEquals(KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY, policy)
            guard(); der.copyOf()
        }
        assertEquals(1, signs); assertContentEquals(der, endpoint.raw)
        assertContentEquals(receipt, prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("Must not sign again") })
        assertContentEquals(receipt, prepared.recoverOriginalApproval())
        receipt.fill(0)
        assertEquals(1, signs); assertContentEquals(endpoint.receipt(), prepared.recoverOriginalApproval())
    }

    @Test fun `retained original recovery consumes without invoking the platform`() {
        val endpoint = Endpoint().apply { state = 2; raw = der.copyOf() }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertContentEquals(endpoint.receipt(), prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("Must recover") })
        assertEquals(3, endpoint.state); assertEquals(0, endpoint.retains)
    }

    @Test fun `uncertain platform outcome freezes rather than asking for another signature`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        var calls = 0
        assertFailsWith<IllegalStateException> {
            prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; error("Platform result lost") }
        }
        assertEquals(1, endpoint.state)
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; der.copyOf() } }
        assertFailsWith<IllegalStateException> { prepared.recoverOriginalApproval() }
        assertEquals(1, calls)
    }

    @Test fun `retention lost return closes old handle and a fresh fixture owner recovers only original`() {
        val endpoint = Endpoint().apply { loseRetainReturn = true }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() } }
        assertEquals(2, endpoint.state); assertEquals(1, endpoint.closes); assertContentEquals(der, endpoint.raw)
        endpoint.loseRetainReturn = false
        val recovered = facade(endpoint).prepareApproval(endpoint.id)
        assertContentEquals(endpoint.receipt(), recovered.performPlatformSigning { _, _, _, _, _, _, _ -> error("No fresh signature") })
    }

    @Test fun `native scope substitution is refused before invocation fence`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        endpoint.substituteScope = true
        assertFailsWith<IllegalStateException> { prepared.signingBytes() }
        assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
    }

    @Test fun `prepared frame rejects key challenge credential subject and legacy C substitutions`() {
        for (index in listOf(4, 6, 8, 13)) {
            val endpoint = Endpoint(); endpoint.fields[index][endpoint.fields[index].lastIndex] =
                (endpoint.fields[index].last().toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareApproval(endpoint.id) }
            assertEquals(1, endpoint.closes); assertEquals(0, endpoint.state)
        }
        for (offset in listOf(155, 323)) {
            val corrupted = Endpoint()
            corrupted.fields[13][offset] = (corrupted.fields[13][offset].toInt() xor 1).toByte()
            // Keep W's SHA(S) correct so only the corrupted enrolled credential/epoch predicate
            // rejects this structurally complete projection before any platform invocation.
            val wStart = "iroha:kagemusha:v1:app-operation-approval\u0000".toByteArray(Charsets.US_ASCII).size + 8
            sha(corrupted.fields[13]).copyInto(corrupted.fields[1], wStart + 3 + 6 * 32)
            assertFailsWith<IllegalArgumentException> { facade(corrupted).prepareApproval(corrupted.id) }
            assertEquals(0, corrupted.state)
        }
        val legacy = Endpoint(); legacy.fields[7] = legacy.fields[7].copyOf(legacy.fields[7].size - 8)
        assertFailsWith<IllegalArgumentException> { facade(legacy).prepareApproval(legacy.id) }
    }

    @Test fun `receipt original substitution cannot be exposed as a completed approval`() {
        val endpoint = Endpoint().apply { state = 2; raw = der.copyOf(); substituteReceipt = true }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.recoverOriginalApproval() }
        assertEquals(1, endpoint.closes)
    }

    @Test fun `E uses distinct purpose pending scope and no financial subject or completed credential`() {
        val endpoint = Endpoint(enrollment = true)
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        assertNull(prepared.recoverOriginalPossession())
        assertContentEquals(endpoint.fields[1], prepared.signingBytes())
        val receipt = prepared.performPlatformSigning { _, _, _, _, message, _, _ ->
            assertContentEquals(endpoint.fields[1], message); der.copyOf()
        }
        assertEquals(2, receipt[10].toInt()); assertContentEquals(endpoint.fields[9], receipt.copyOfRange(147, 179))
        val wrong = Endpoint(enrollment = true); wrong.fields[8] = bytes(7)
        assertFailsWith<IllegalArgumentException> { facade(wrong).prepareEnrollmentPossession(wrong.id) }
        val renewed = Endpoint(enrollment = true)
        val times = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8 + 355
        ByteBuffer.wrap(renewed.fields[1]).order(ByteOrder.LITTLE_ENDIAN).putLong(times, 1001)
        assertFailsWith<IllegalArgumentException> { facade(renewed).prepareEnrollmentPossession(renewed.id) }
        val apple = Endpoint(enrollment = true)
        val cBody = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII).size + 8
        apple.fields[7][cBody + 2] = 2
        apple.fields[2] = byteArrayOf(4); apple.fields[4] = sha(apple.fields[7])
        val eBody = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8
        apple.id.copyInto(apple.fields[1], eBody + 3)
        apple.fields[3] = java.util.Base64.getEncoder().encodeToString(apple.fields[6]).toByteArray(Charsets.US_ASCII)
        apple.fields[10] = KagemushaCoreCoordinatorFrameV1.u32(0); apple.fields[11] = byteArrayOf(0)
        val applePrepared = facade(apple).prepareEnrollmentPossession(apple.id)
        assertContentEquals(apple.fields[1], applePrepared.signingBytes())
        assertFailsWith<IllegalStateException> { applePrepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("No Android invocation") } }
        assertEquals(0, apple.state)
        val subject = Endpoint(enrollment = true); subject.fields[13] = byteArrayOf(1)
        assertFailsWith<IllegalArgumentException> { facade(subject).prepareEnrollmentPossession(subject.id) }
    }

    @Test fun `E rejects stable enrollment ID and a second challenge hash before signing`() {
        val eBody = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8
        val cBody = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII).size + 8
        for (stableId in listOf(true, false)) {
            val endpoint = Endpoint(enrollment = true)
            val substituted = if (stableId) endpoint.fields[7].copyOfRange(cBody + 3, cBody + 35)
                else sha(sha(endpoint.fields[7]))
            // Correlate the offered operation and E to each other so only the sole full-C
            // binding rejects this alternate. No native/platform invocation is represented.
            endpoint.offeredId = substituted.copyOf()
            substituted.copyInto(endpoint.fields[1], eBody + 3)
            assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareEnrollmentPossession(substituted) }
            assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
        }
    }

    @Test fun `E original evidence reads only consumed native originals without invoking or consuming`() {
        val endpoint = Endpoint(enrollment = true)
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.originalPlatformEvidence() }
        assertEquals(0, endpoint.state); assertEquals(0, endpoint.consumes)
        endpoint.state = 2; endpoint.raw = der.copyOf()
        assertFailsWith<IllegalStateException> { prepared.originalPlatformEvidence() }
        assertEquals(2, endpoint.state); assertEquals(0, endpoint.consumes)
        prepared.recoverOriginalPossession()
        assertEquals(1, endpoint.consumes)
        val original = prepared.originalPlatformEvidence()
        assertContentEquals(der, original)
        original.fill(0)
        assertContentEquals(der, prepared.originalPlatformEvidence())
        assertEquals(1, endpoint.consumes); assertEquals(0, endpoint.retains)
    }

    @Test fun `E original evidence substitution closes the held ticket before data exposure`() {
        val endpoint = Endpoint(enrollment = true).apply { state = 3; raw = der.copyOf() }
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        endpoint.substituteReceipt = true
        assertFailsWith<IllegalStateException> { prepared.originalPlatformEvidence() }
        assertEquals(1, endpoint.closes); assertContentEquals(der, endpoint.raw)
        assertFailsWith<IllegalStateException> { prepared.originalPlatformEvidence() }
    }

    @Test fun `E opaque credential intake retains exact bytes and retries a lost original result`() {
        val endpoint = Endpoint(enrollment = true).apply { state = 3; raw = der.copyOf(); loseCredentialReturn = true }
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        val original = byteArrayOf(0x71, 0x72, 0x73) // Inert scripted opaque carrier, never a native credential.
        assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(original) }
        assertContentEquals(original, endpoint.credential); assertEquals(1, endpoint.closes)
        assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(original) }
        endpoint.loseCredentialReturn = false
        val recovered = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        val digest = recovered.acceptOriginalCredential(original)
        assertContentEquals(sha(original), digest)
        original.fill(0); digest.fill(0)
        assertContentEquals(byteArrayOf(0x71, 0x72, 0x73), endpoint.credential)
        assertContentEquals(sha(endpoint.credential), recovered.acceptOriginalCredential(endpoint.credential.copyOf()))
        assertEquals(3, endpoint.state); assertEquals(0, endpoint.consumes); assertEquals(0, endpoint.retains)
        assertFailsWith<IllegalStateException> { recovered.acceptOriginalCredential(byteArrayOf(0x74)) }
        assertContentEquals(byteArrayOf(0x71, 0x72, 0x73), endpoint.credential)
    }

    @Test fun `E credential scope substitution cannot become a completed native identity`() {
        val endpoint = Endpoint(enrollment = true).apply { state = 3; raw = der.copyOf(); substituteCredentialScope = true }
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(byteArrayOf(1)) }
        assertEquals(1, endpoint.closes); assertContentEquals(byteArrayOf(1), endpoint.credential)
    }

    @Test fun `E credential phase eight has exact widths and cannot be used by monetary W`() {
        val e = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION
        val w = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL
        val fields = listOf(KagemushaCoreCoordinatorFrameV1.u32(8), le64(7), ByteArray(16 * 1024) { 1 })
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(e, fields)
        assertContentEquals(fields[2], KagemushaCoreCoordinatorFrameV1.decodeRequest(e, request)[2])
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(w, fields) }
        for (bad in listOf(fields.dropLast(1), fields + listOf(byteArrayOf(1)),
            listOf(fields[0], ByteArray(32) { 1 }, fields[2]),
            listOf(fields[0], fields[1], byteArrayOf()),
            listOf(fields[0], fields[1], ByteArray(16 * 1024 + 1)))) {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(e, bad) }
        }
        val response = listOf(bytes(1), bytes(2))
        KagemushaCoreCoordinatorFrameV1.encodeResponse(e, request, response)
        for (bad in listOf(response.dropLast(1), response + listOf(bytes(3)), listOf(ByteArray(32), response[1]))) {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(e, request, bad) }
        }
    }

    @Test fun `completed original frame rejects substituted raw evidence and a zero Apple counter`() {
        val endpoint = Endpoint(enrollment = true).apply { state = 3; raw = der.copyOf() }
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(5), endpoint.fields[0]))
        val receipt = endpoint.receipt()
        val alternateRaw = der.copyOf().apply { this[lastIndex] = 2 }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request,
                listOf(byteArrayOf(2), alternateRaw, receipt))
        }
        receipt[179] = 1
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request,
                listOf(byteArrayOf(2), der.copyOf(), receipt))
        }
    }

    @Test fun `E credential intake refuses unconsumed native states without signing or retention`() {
        for (nativeState in listOf(0, 1, 2)) {
            val endpoint = Endpoint(enrollment = true).apply {
                state = nativeState; if (nativeState == 2) raw = der.copyOf()
            }
            val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
            assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(byteArrayOf(1)) }
            assertEquals(nativeState, endpoint.state)
            assertEquals(0, endpoint.retains); assertEquals(0, endpoint.consumes)
            assertEquals(0, endpoint.credential.size); assertEquals(1, endpoint.closes)
            assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(byteArrayOf(1)) }
        }
    }

    @Test fun `E credential intake rejects malformed native returns and a changed fresh postcheck`() {
        for (bad in listOf(arrayOf(bytes(1)), arrayOf(bytes(1), ByteArray(32)),
            arrayOf(bytes(1), bytes(0x99), bytes(3)), arrayOf(ByteArray(31), bytes(0x99)))) {
            val endpoint = Endpoint(enrollment = true).apply {
                state = 3; raw = der.copyOf(); credentialResponseOverride = bad
            }
            val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
            assertFailsWith<IllegalArgumentException> { prepared.acceptOriginalCredential(byteArrayOf(1)) }
            assertEquals(0, endpoint.consumes); assertEquals(0, endpoint.retains)
            assertEquals(1, endpoint.closes)
        }
        val endpoint = Endpoint(enrollment = true).apply {
            state = 3; raw = der.copyOf(); substituteScopeAfterCredential = true
        }
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.acceptOriginalCredential(byteArrayOf(1)) }
        assertEquals(1, endpoint.closes); assertEquals(0, endpoint.consumes); assertEquals(0, endpoint.retains)
    }

    @Test fun `Bootstrap has a separate strict full projection and generic money rejects it`() {
        val endpoint = Endpoint(bootstrap = true)
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL
        val bootstrapRequest = listOf(KagemushaCoreCoordinatorFrameV1.u32(8), endpoint.id)
        val request = KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalRequest(bootstrapRequest)
        assertContentEquals(endpoint.id, KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalRequest(request)[1])
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, bootstrapRequest) }
        for (invalid in listOf(bootstrapRequest.dropLast(1), bootstrapRequest + listOf(byteArrayOf(1)),
            listOf(bootstrapRequest[0], ByteArray(32)), listOf(bootstrapRequest[0], le64(7)))) {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalRequest(invalid) }
        }
        val genericRequest = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(1), endpoint.id))
        assertFailsWith<IllegalStateException> { KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalRequest(genericRequest) }
        val encoded = KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, endpoint.fields)
        val decoded = KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalResponse(request, encoded)
        assertContentEquals(endpoint.fields[13], decoded[13])
        decoded[13].fill(0)
        assertContentEquals(endpoint.fields[13], KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalResponse(request, encoded)[13])
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, genericRequest, endpoint.fields) }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, encoded) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, Endpoint().fields)
        }
        val invalidPhase = KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(8), le64(7), byteArrayOf(1)))
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(invalidPhase, listOf(bytes(1), bytes(2))) }
    }

    @Test fun `Bootstrap rejects operation relabel indices outgoing commitments and missing bindings before signing`() {
        for (offset in listOf(331, 364, 396, 428, 444, 59, 91, 123, 155, 187, 219, 251, 283, 291, 323, 332)) {
            val endpoint = Endpoint(bootstrap = true)
            val s = endpoint.fields[13]
            if (offset in listOf(331, 364, 396, 428, 444)) s[offset] = 1
            else s.fill(0, offset, offset + if (offset in listOf(283, 323)) 8 else 32)
            val body = "iroha:kagemusha:v1:app-operation-approval\u0000".toByteArray(Charsets.US_ASCII).size + 8
            sha(s).copyInto(endpoint.fields[1], body + 3 + 6 * 32)
            assertFailsWith<IllegalArgumentException> { bootstrap(endpoint) }
            assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
        }
    }

    @Test fun `Bootstrap hardware fence retains exact DER and repeated capture never resigns`() {
        val endpoint = Endpoint(bootstrap = true); var ownerChecks = 0; var signs = 0
        val prepared = bootstrap(endpoint) { ownerChecks++ }
        val receipt = prepared.performPlatformSigning { alias, c, point, pointDigest, w, _, guard ->
            signs++; assertEquals(1, endpoint.state); guard()
            assertEquals(endpoint.fields[3].toString(Charsets.UTF_8), alias)
            assertContentEquals(endpoint.fields[4], c); assertContentEquals(endpoint.fields[5], point)
            assertContentEquals(endpoint.fields[6], pointDigest); assertContentEquals(endpoint.fields[1], w)
            der.copyOf()
        }
        assertContentEquals(der, endpoint.raw); assertEquals(1, endpoint.retains)
        assertContentEquals(receipt, prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("No second signature") })
        assertContentEquals(receipt, prepared.recoverOriginalApproval()); assertEquals(1, signs)
        kotlin.test.assertTrue(ownerChecks > 3)
        val detached = prepared.bootstrapSelectionOriginal(); detached.fill(0)
        assertContentEquals(endpoint.fields[13], prepared.bootstrapSelectionOriginal())
    }

    @Test fun `Bootstrap lost retain return allows only original recovery in a fresh holder`() {
        val endpoint = Endpoint(bootstrap = true).apply { loseRetainReturn = true }; var signs = 0
        val old = bootstrap(endpoint)
        assertFailsWith<IllegalStateException> { old.performPlatformSigning { _, _, _, _, _, _, _ -> signs++; der.copyOf() } }
        assertEquals(2, endpoint.state); assertEquals(1, endpoint.closes)
        assertFailsWith<IllegalStateException> { old.recoverOriginalApproval() }
        endpoint.loseRetainReturn = false
        assertContentEquals(endpoint.receipt(), bootstrap(endpoint).performPlatformSigning { _, _, _, _, _, _, _ -> error("No fresh signature") })
        assertEquals(1, signs); assertEquals(1, endpoint.retains)
    }

    @Test fun `Bootstrap original owner revocation scope substitution and receipt substitution stop the holder`() {
        var current = true
        val revoked = Endpoint(bootstrap = true); val prepared = bootstrap(revoked) { check(current) }
        current = false
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("No OS invocation") } }
        assertEquals(0, revoked.state); assertEquals(1, revoked.closes)
        for (receipt in listOf(false, true)) {
            val endpoint = Endpoint(bootstrap = true)
            val held = bootstrap(endpoint)
            if (receipt) { endpoint.state = 2; endpoint.raw = der.copyOf(); endpoint.substituteReceipt = true }
            else endpoint.substituteScope = true
            assertFailsWith<IllegalStateException> { held.recoverOriginalApproval() }
            assertEquals(1, endpoint.closes)
        }
    }

    @Test fun `Bootstrap cancellation and uncertain OS outcomes cannot create another invocation`() {
        val cancelled = Endpoint(bootstrap = true); val held = bootstrap(cancelled)
        held.cancel()
        assertFailsWith<IllegalStateException> { held.signingBytes() }
        assertEquals(0, cancelled.state)
        val uncertain = Endpoint(bootstrap = true); val prepared = bootstrap(uncertain); var calls = 0
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; error("OS result lost") } }
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; der.copyOf() } }
        assertEquals(1, uncertain.state); assertEquals(1, calls)
    }

    @Test fun `initial publication uses captured W and repeated publication and recovery never resign`() {
        val endpoint = Endpoint(bootstrap = true); var signs = 0
        val held = bootstrap(endpoint)
        held.performPlatformSigning { _, _, _, _, _, _, _ -> signs++; der.copyOf() }
        val publication = held.publishOriginalInitialState()
        assertContentEquals(endpoint.publication[1], publication.enrollmentId())
        assertContentEquals(endpoint.publication[3], publication.retailCertificateOriginalDigest())
        assertContentEquals(endpoint.fields[8], publication.appCredentialOriginalDigest())
        assertContentEquals(endpoint.publication[5], publication.bootstrapApprovalOriginalDigest())
        assertContentEquals(endpoint.publication[6], publication.stateOriginalDigest())
        assertContentEquals(endpoint.publication[7], publication.pairedStateProofOriginalDigest())
        assertContentEquals(endpoint.publication[8], publication.pairedOrdinaryGuardOriginalDigest())
        publication.publicationOriginalDigest().fill(0)
        assertContentEquals(endpoint.publication[2], publication.publicationOriginalDigest())
        assertContentEquals(endpoint.publication[2], held.publishOriginalInitialState().publicationOriginalDigest())
        assertContentEquals(endpoint.publication[2], held.recoverOriginalInitialStatePublication().publicationOriginalDigest())
        assertEquals(1, endpoint.proofs); assertEquals(2, endpoint.publicationCalls); assertEquals(1, endpoint.publicationRecoveries)
        assertEquals(1, signs); assertEquals(1, endpoint.retains)
    }

    @Test fun `initial publication rejects missing captured W or missing native FI binding before proof dispatch`() {
        val endpoint = Endpoint(bootstrap = true)
        assertFailsWith<IllegalStateException> { bootstrap(endpoint).publishOriginalInitialState() }
        assertEquals(0, endpoint.publicationCalls); assertEquals(0, endpoint.state)
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/native-bootstrap-unbound", endpoint)
        val fields = bridge.invokeOrdinaryBootstrapApproval(listOf(KagemushaCoreCoordinatorFrameV1.u32(8), endpoint.id))
        val unbound = KagemushaNativePreparedOrdinaryBootstrapApprovalV1.fromNative(bridge, endpoint.id, fields, {})
        assertFailsWith<IllegalStateException> { unbound.publishOriginalInitialState() }
        assertEquals(0, endpoint.publicationCalls)
    }

    @Test fun `initial publication cannot substitute ticket FI enrollment certificate or credential`() {
        for (index in listOf(0, 1, 3, 4)) {
            val endpoint = Endpoint(bootstrap = true)
            val held = bootstrap(endpoint)
            held.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() }
            endpoint.publication[index] = if (index == 0) le64(8) else bytes(0x74)
            assertFailsWith<RuntimeException> { held.publishOriginalInitialState() }
            assertFailsWith<IllegalStateException> { held.recoverOriginalInitialStatePublication() }
            assertEquals(1, endpoint.closes); assertEquals(1, endpoint.retains)
        }
    }

    @Test fun `initial publication rechecks every retained original commitment on recovery`() {
        for (index in listOf(2, 5, 6, 7, 8)) {
            val endpoint = Endpoint(bootstrap = true)
            val held = bootstrap(endpoint)
            held.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() }
            held.publishOriginalInitialState()
            endpoint.publication[index] = bytes(0x74)
            assertFailsWith<IllegalStateException> { held.recoverOriginalInitialStatePublication() }
            assertEquals(1, endpoint.closes); assertEquals(1, endpoint.proofs); assertEquals(1, endpoint.retains)
        }
    }

    @Test fun `lost initial publication return allows only native original recovery from a fresh holder`() {
        val endpoint = Endpoint(bootstrap = true).apply { losePublicationReturn = true }; var signs = 0
        val old = bootstrap(endpoint)
        old.performPlatformSigning { _, _, _, _, _, _, _ -> signs++; der.copyOf() }
        assertFailsWith<IllegalStateException> { old.publishOriginalInitialState() }
        assertEquals(1, endpoint.proofs); assertEquals(1, endpoint.closes)
        assertFailsWith<IllegalStateException> { old.publishOriginalInitialState() }
        endpoint.losePublicationReturn = false
        val recovered = bootstrap(endpoint).recoverOriginalInitialStatePublication()
        assertContentEquals(endpoint.publication[2], recovered.publicationOriginalDigest())
        assertEquals(1, endpoint.proofs); assertEquals(1, endpoint.publicationCalls)
        assertEquals(1, signs); assertEquals(1, endpoint.retains)
    }

    @Test fun `missing native initial publication provider stays unavailable without a money claim`() {
        val endpoint = Endpoint(bootstrap = true).apply { publicationUnavailable = true }
        val held = bootstrap(endpoint)
        held.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() }
        assertFailsWith<IllegalStateException> { held.publishOriginalInitialState() }
        assertEquals(0, endpoint.proofs); assertEquals(1, endpoint.closes)
        assertFailsWith<IllegalStateException> { held.recoverOriginalInitialStatePublication() }
    }

    @Test fun `publication recovery cannot create a new proof and original owner revocation stops dispatch`() {
        val missing = Endpoint(bootstrap = true)
        val held = bootstrap(missing)
        held.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() }
        assertFailsWith<IllegalStateException> { held.recoverOriginalInitialStatePublication() }
        assertEquals(0, missing.proofs); assertEquals(0, missing.publicationCalls)
        var current = true
        val revoked = Endpoint(bootstrap = true)
        val guarded = bootstrap(revoked) { check(current) }
        guarded.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() }
        current = false
        assertFailsWith<IllegalStateException> { guarded.publishOriginalInitialState() }
        assertEquals(0, revoked.publicationCalls); assertEquals(1, revoked.closes)
    }

    private fun bootstrap(endpoint: Endpoint, guard: () -> Unit = {}): KagemushaNativePreparedOrdinaryBootstrapApprovalV1 {
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/native-bootstrap-owner", endpoint)
        val fields = bridge.invokeOrdinaryBootstrapApproval(listOf(KagemushaCoreCoordinatorFrameV1.u32(8), endpoint.id))
        return KagemushaNativePreparedOrdinaryBootstrapApprovalV1.fromNative(bridge, endpoint.id, fields, guard,
            endpoint.enrollmentId, endpoint.retailCertificate)
    }

    private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
        KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/native-owner", endpoint))

    private class Endpoint(val enrollment: Boolean = false, val bootstrap: Boolean = false) : KagemushaCoreCoordinatorEndpointV1 {
        val fields = projection(enrollment, bootstrap).toMutableList()
        var offeredId: ByteArray? = null
        val id: ByteArray get() = offeredId?.copyOf() ?: if (enrollment) sha(fields[7]) else bytes(0x11)
        var state = 0 // 0 uninvoked, 1 invoked/no original, 2 retained, 3 consumed
        var raw = byteArrayOf(); var retains = 0; var closes = 0; var consumes = 0
        var credential = byteArrayOf(); var loseCredentialReturn = false; var substituteCredentialScope = false
        var credentialResponseOverride: Array<ByteArray>? = null; var substituteScopeAfterCredential = false
        var loseRetainReturn = false; var substituteScope = false; var substituteReceipt = false
        val enrollmentId = bytes(0xb1)
        val retailCertificate = byteArrayOf(0xb2.toByte(), 1)
        // Scripted transport commitments exercise correlation; they are not genuine proof evidence.
        val publication = mutableListOf(fields[0].copyOf(), enrollmentId.copyOf(), bytes(0xb3), sha(retailCertificate),
            fields[8].copyOf(), bytes(0xb4), bytes(0xb5), bytes(0xb6), bytes(0xb7))
        var proofs = 0; var publicationCalls = 0; var publicationRecoveries = 0
        var published = false; var losePublicationReturn = false; var publicationUnavailable = false
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, request: Array<ByteArray>): Array<ByteArray>? {
            assertEquals(if (enrollment) 20 else 19, method)
            val phase = ByteBuffer.wrap(request[0]).order(ByteOrder.LITTLE_ENDIAN).int
            if (phase != (if (bootstrap) 8 else 1)) assertContentEquals(fields[0], request[1])
            if (bootstrap && phase == 8) { assertContentEquals(id, request[1]); return fields.map(ByteArray::copyOf).toTypedArray() }
            return when (phase) {
                1 -> { check(!bootstrap); assertContentEquals(id, request[1]); fields.map(ByteArray::copyOf).toTypedArray() }
                2 -> when (state) {
                    0 -> { state = 1; arrayOf(byteArrayOf(1), byteArrayOf(), byteArrayOf()) }
                    2 -> arrayOf(byteArrayOf(2), raw.copyOf(), byteArrayOf())
                    3 -> arrayOf(byteArrayOf(3), raw.copyOf(), receipt())
                    else -> null
                }
                3 -> { check(state == 1); retains++; raw = request[2].copyOf(); state = 2
                    if (loseRetainReturn) null else arrayOf(sha(raw)) }
                4 -> { check(state == 2 || state == 3); consumes++; state = 3; arrayOf(receipt()) }
                5 -> when (state) {
                    0 -> arrayOf(byteArrayOf(0), byteArrayOf(), byteArrayOf())
                    2 -> arrayOf(byteArrayOf(1), raw.copyOf(), byteArrayOf())
                    3 -> arrayOf(byteArrayOf(2), raw.copyOf(), receipt())
                    else -> null
                }
                6 -> arrayOf(if (substituteScope) bytes(0x42) else fields[9].copyOf(), sha(fields[1]))
                7 -> emptyArray()
                8 -> {
                    check(enrollment && state == 3)
                    if (credential.isNotEmpty() && !credential.contentEquals(request[2])) null
                    else {
                        credential = request[2].copyOf()
                        if (loseCredentialReturn) null else {
                            val response = credentialResponseOverride ?: arrayOf(sha(credential),
                                if (substituteCredentialScope) bytes(0x42) else fields[9].copyOf())
                            if (substituteScopeAfterCredential) substituteScope = true
                            response
                        }
                    }
                }
                9 -> {
                    check(bootstrap && state == 3); publicationCalls++
                    if (publicationUnavailable) null else {
                        if (!published) { published = true; proofs++ }
                        if (losePublicationReturn) null else publication.map(ByteArray::copyOf).toTypedArray()
                    }
                }
                10 -> {
                    check(bootstrap && state == 3); publicationRecoveries++
                    if (!published) null else publication.map(ByteArray::copyOf).toTypedArray()
                }
                else -> error("Unexpected fixture phase")
            }
        }
        fun receipt(): ByteArray = output {
            write("KGMAPP1\u0000".toByteArray(Charsets.US_ASCII)); write(byteArrayOf(1, 0, if (enrollment) 2 else 1))
            write(fields[0]); write(if (substituteReceipt) bytes(0x77) else id); write(fields[9]); write(sha(fields[1]))
            write(sha(raw)); write(if (enrollment) fields[9] else fields[8]); write(ByteArray(5))
        }
    }
    companion object {
        private val der = byteArrayOf(0x30, 6, 2, 1, 1, 2, 1, 1)
        private fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        private fun sha(value: ByteArray) = MessageDigest.getInstance("SHA-256").digest(value)
        private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        private fun output(block: ByteArrayOutputStream.() -> Unit) = ByteArrayOutputStream().apply(block).toByteArray()
        private fun framed(domain: String, body: ByteArray) = output {
            write((domain + '\u0000').toByteArray(Charsets.US_ASCII)); write(le64(body.size.toLong())); write(body)
        }
        private fun projection(enrollment: Boolean, bootstrap: Boolean = false): List<ByteArray> {
            val pair = KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }.generateKeyPair()
            val public = pair.public as ECPublicKey
            fun coordinate(value: BigInteger) = value.toByteArray().takeLast(32).toByteArray().let { ByteArray(32 - it.size) + it }
            val point = byteArrayOf(4) + coordinate(public.w.affineX) + coordinate(public.w.affineY); val key = sha(point)
            val cFields = (1..13).map(::bytes)
            val c = framed("iroha:kagemusha:v1:ordinary-app-enrollment-challenge", output {
                write(byteArrayOf(1, 0, 1)); cFields.forEach { write(it) }
                write(le64(1)); write(le64(1)); write(le64(1000)); write(le64(121000))
            })
            val s = framed("iroha:kagemusha:v1:hardware-transition-selection", output {
                write(byteArrayOf(1, 0)); write(cFields[6]); write(bytes(0x61)); write(bytes(0x62)); write(bytes(0x66))
                write(cFields[4]); write(cFields[5]); write(cFields[7]); write(le64(1)); write(bytes(0x64)); write(le64(1))
                write(byteArrayOf(if (bootstrap) 0 else 1)); write(bytes(0x65)); write(ByteArray(64)); write(le64(if (bootstrap) 0 else 7)); write(le64(0)); write(le64(if (bootstrap) 0 else 8)); write(le64(0))
            })
            val credential = bytes(0x66)
            val subjectFields = if (enrollment) listOf(sha(c), cFields[1], cFields[2], cFields[3], cFields[4],
                cFields[10], cFields[6], cFields[7], cFields[5], key, bytes(0x77)) else listOf(bytes(0x11), bytes(0x22),
                cFields[3], cFields[10], key, credential, sha(s), bytes(0x88))
            val message = framed(if (enrollment) "iroha:kagemusha:v1:app-enrollment-possession" else "iroha:kagemusha:v1:app-operation-approval", output {
                write(byteArrayOf(1, 0, 1)); subjectFields.forEach { write(it) }; write(le64(1000)); write(le64(121000))
            })
            return listOf(le64(7), message, byteArrayOf(5), KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(Charsets.UTF_8), sha(c), point, key, c,
                if (enrollment) byteArrayOf() else credential, bytes(0x99), byteArrayOf(), byteArrayOf(1), bytes(0xaa),
                if (enrollment) byteArrayOf() else s)
        }
    }
}
