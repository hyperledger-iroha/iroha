// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.util.Base64
import java.util.concurrent.CompletableFuture
import kotlinx.coroutines.runBlocking
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityProviderV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityTokenOriginalV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaPlayIntegrityBackendV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaPlayIntegrityPreparedV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.KagemushaAndroidKeyAttestationArchiveV1
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Scripted private-holder pairing and original HTTP diagnostics; no Native/device/issuer qualification. */
class KagemushaOrdinaryCertificateHttpOriginalV1Test {
    @Test fun certificateUsesOnlyTheSameNativeReservationIdentityAndConsumedEOriginals() {
        val e = Endpoint(); val held = Held(e)
        val request = held.e.certificateRequestOriginal(held.reservation, held.identity)
        assertEquals("/v1/offline/enrollment/ordinary/certificate", request.path)
        assertEquals(KagemushaOrdinaryIdentityHttpCodecV1.certificateRequestId(e.attempt), request.requestId)
        assertNotEquals(KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(e.attempt), request.requestId)
        val body = fields(request.body())
        assertEquals(setOf("schema", "operation", "operation_id", "signed_preparation_base64", "attested_public_key_sec1_base64",
            "raw_attestation_base64", "app_possession", "play_integrity_token"), body.keys)
        assertEquals(hex(e.attempt), body["operation_id"])
        assertEquals(base64(e.signedC), body["signed_preparation_base64"])
        assertEquals(base64(e.point), body["attested_public_key_sec1_base64"])
        assertEquals(base64(e.raw), body["raw_attestation_base64"])
        assertNull(body["play_integrity_token"])
        @Suppress("UNCHECKED_CAST") val pop = body["app_possession"] as Map<String, Any?>
        assertEquals(mapOf("platform" to "android_keystore", "signature_der_base64" to base64(der)), pop)
        request.body().fill(0); assertEquals(body, fields(request.body()))
        assertEquals(0, e.credentialIntakes)
        assertTrue(e.phases.all { it != 2 && it != 3 && it != 4 && it != 9 })
    }

    @Test fun anotherCoordinatorOrChangedNativeRawScopeCannotSupplyCertificateInputs() {
        val first = Held(Endpoint()); val other = Held(Endpoint())
        assertFailsWith<IllegalStateException> { first.e.certificateRequestOriginal(other.reservation, other.identity) }
        val e = Endpoint(); val held = Held(e); e.changedScope = true
        assertFailsWith<IllegalStateException> { held.e.certificateRequestOriginal(held.reservation, held.identity) }
        assertEquals(0, e.credentialIntakes); assertTrue(e.closes > 0)
    }

    @Test fun originalRawHashAndPossessionSignatureCannotBeSubstitutedByAnotherRetainedFixture() {
        for (rawChanged in listOf(true, false)) {
            val e = Endpoint(); val held = Held(e)
            if (rawChanged) e.raw = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(listOf(
                byteArrayOf(0x30, 2, 1, 9), byteArrayOf(0x30, 2, 1, 2))).transportBytes()
            else e.changedReceipt = true
            assertFails { held.e.certificateRequestOriginal(held.reservation, held.identity) }
            assertEquals(0, e.credentialIntakes); assertTrue(e.closes > 0)
        }
    }

    @Test fun mandatoryPiUsesNativePolicyProjectAndSoleFullCKeyHash() {
        val e = Endpoint(policyOriginal = policy()); val held = Held(e)
        assertFailsWith<IllegalStateException> { held.e.certificateRequestOriginal(held.reservation, held.identity) }
        assertFailsWith<IllegalArgumentException> {
            held.e.certificateRequestOriginal(held.reservation, held.identity, token(e, project = 8))
        }
        val request = held.e.certificateRequestOriginal(held.reservation, held.identity, token(e))
        assertEquals("original-opaque-google-token", fields(request.body())["play_integrity_token"])
        assertFailsWith<IllegalStateException> {
            held.e.certificateRequestOriginal(held.reservation, held.identity, token(e, opaque = "different-google-token"))
        }
        val wrongHash = Endpoint(policyOriginal = policy()); val bad = Held(wrongHash)
        assertFailsWith<IllegalStateException> {
            bad.e.certificateRequestOriginal(bad.reservation, bad.identity,
                KagemushaAndroidPlayIntegrityTokenOriginalV1(7, sha(wrongHash.attempt), "opaque-token"))
        }
        val absent = Endpoint(); val noPi = Held(absent)
        assertFailsWith<IllegalArgumentException> { noPi.e.certificateRequestOriginal(noPi.reservation, noPi.identity, token(absent)) }
    }

    @Test fun googleProviderReceivesOnlySelectedProjectAndCanonicalNativeRequestHash() {
        val e = Endpoint(policyOriginal = policy()); val held = Held(e)
        var calls = 0
        val backend = object : KagemushaPlayIntegrityBackendV1 {
            override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> {
                assertEquals(7L, cloudProjectNumber)
                return CompletableFuture.completedFuture(object : KagemushaPlayIntegrityPreparedV1 {
                    override fun request(originalHashText: String): CompletableFuture<String> {
                        calls++
                        assertEquals(Base64.getUrlEncoder().withoutPadding().encodeToString(token(e).requestHash()), originalHashText)
                        assertEquals(43, originalHashText.length)
                        return CompletableFuture.completedFuture("original-opaque-google-token")
                    }
                })
            }
        }
        val original = held.e.requestOriginalIntegrityToken(held.reservation, held.identity,
            KagemushaAndroidPlayIntegrityProviderV1(backend)).join()
        assertEquals(1, calls); assertEquals(7L, original.cloudProjectNumber)
        val carrier = held.e.certificateRequestOriginal(held.reservation, held.identity, original)
        e.policyOriginal = policy(number = 8)
        assertFailsWith<IllegalStateException> { carrier.body() }
        assertEquals(0, e.credentialIntakes)
    }

    @Test fun protectedCertificateResponseRemainsUntrustedUntilNativeIntake() = runBlocking {
        val e = Endpoint(); val held = Held(e)
        val opaque = byteArrayOf(0x71, 0x72) // Inert transport original, not an admitted Native credential.
        val digest = held.e.issueOriginalCredential(held.reservation, held.identity, null) { request ->
            request.requireCurrent(); assertEquals(0, e.credentialIntakes)
            json(linkedMapOf("certificate_base64" to base64(opaque), "certificate_sha256_hex" to hex(sha(opaque))))
        }
        assertEquals(1, e.credentialIntakes); assertContentEquals(opaque, e.credential)
        assertContentEquals(sha(opaque), digest)
        for (mutation in listOf("digest", "extra", "empty")) {
            val bad = Endpoint(); val badHeld = Held(bad)
            assertFails { badHeld.e.issueOriginalCredential(badHeld.reservation, badHeld.identity, null) {
                val bytes = if (mutation == "empty") byteArrayOf() else opaque
                val fields = linkedMapOf<String, Any?>("certificate_base64" to base64(bytes),
                    "certificate_sha256_hex" to hex(if (mutation == "digest") bytes(4) else sha(bytes)))
                if (mutation == "extra") fields["qualified"] = true
                json(fields)
            } }
            assertEquals(0, bad.credentialIntakes)
        }
    }

    @Test fun currentOwnerChangeDuringHttpStopsBeforeCredentialIntake() = runBlocking {
        val e = Endpoint(); val held = Held(e)
        assertFailsWith<IllegalStateException> {
            held.e.issueOriginalCredential(held.reservation, held.identity, null) { request ->
                request.requireCurrent(); e.changedScope = true
                json(linkedMapOf("certificate_base64" to base64(byteArrayOf(1)), "certificate_sha256_hex" to hex(sha(byteArrayOf(1)))))
            }
        }
        assertEquals(0, e.credentialIntakes)
    }

    @Test fun pureSelectedGoogleProjectionRejectsExtraMembersUnsupportedNumbersAndBrokenIdentitySyntax() {
        assertEquals(7L, KagemushaOrdinaryIdentityHttpCodecV1.playIntegrityCloudProjectOriginal(policy()))
        for (mutation in listOf("extra", "project-extra", "project-zero", "project-overflow", "principal", "certificate", "version")) {
            val f = fields(policy()).toMutableMap()
            when (mutation) {
                "extra" -> f["verdict"] = true
                "version" -> f["version"] = 2L
                "principal" -> f["credentialSubject"] = mapOf("email" to "operator@foreign.iam.gserviceaccount.com", "clientId" to "123")
                "certificate" -> f["appSigningCertificateSha256Hex"] = "A".repeat(64)
                else -> {
                    @Suppress("UNCHECKED_CAST") val p = (f["cloudProject"] as Map<String, Any?>).toMutableMap()
                    when (mutation) {
                        "project-extra" -> p["selected"] = true
                        "project-zero" -> p["number"] = 0L
                        else -> p["number"] = java.math.BigInteger.ONE.shiftLeft(63)
                    }
                    f["cloudProject"] = p
                }
            }
            assertFails { KagemushaOrdinaryIdentityHttpCodecV1.playIntegrityCloudProjectOriginal(json(f)) }
        }
    }

    private class Held(val endpoint: Endpoint) {
        private val facade = KagemushaNativeAppApprovalCoordinatorV1(KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/native-paired-carrier", endpoint))
        val reservation = facade.reserveOriginalIdentity()
        val identity = reservation.acceptOriginalSignedPreparation(endpoint.signedC)
        val e = facade.prepareEnrollmentPossession(endpoint.attempt)
    }

    private class Endpoint(var policyOriginal: ByteArray = byteArrayOf()) : KagemushaCoreCoordinatorEndpointV1 {
        val signedC = byteArrayOf(1, 0, 1) + (1..13).flatMap { bytes(it).toList() }.toByteArray() +
            le64(1) + le64(2) + le64(1000) + le64(121000) + ByteArray(64) { 0x31 }
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
        val attempt = c.attestationChallenge()
        val point: ByteArray = KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }
            .generateKeyPair().public.let { it as ECPublicKey }.let {
                fun coordinate(n: java.math.BigInteger) = n.toByteArray().takeLast(32).toByteArray().let { b -> ByteArray(32 - b.size) + b }
                byteArrayOf(4) + coordinate(it.w.affineX) + coordinate(it.w.affineY)
            }
        var raw = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(listOf(
            byteArrayOf(0x30, 2, 1, 1), byteArrayOf(0x30, 2, 1, 2))).transportBytes()
        val originalRaw = raw.copyOf(); val pending = bytes(0x40)
        val alias = KagemushaOrdinaryAppKeyAliasV1.originalAlias(c.canonicalSigningBytes()).toByteArray()
        val reserved = listOf(le64(6), "fixture-account".toByteArray(), c.clientNonce(), c.releaseId(), c.hardwareProfileId(),
            c.laneId(), c.financialAuthorityCommitment(), KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(c.clientNonce()).toByteArray())
        val prepared = listOf(le64(7), signedC, c.canonicalSigningBytes(), attempt, byteArrayOf(5), alias, byteArrayOf(1), bytes(0x20))
        val eMessage = framed("iroha:kagemusha:v1:app-enrollment-possession", byteArrayOf(1, 0, 1) + listOf(
            attempt, c.clientNonce(), c.serverNonce(), c.accountBinding(), c.networkId(), c.appAuthorityPolicyDigest(), c.releaseId(),
            c.hardwareProfileId(), c.laneId(), sha(point), sha(originalRaw)).flatMap { it.toList() }.toByteArray() + le64(1000) + le64(121000))
        val eFields = listOf(le64(8), eMessage, byteArrayOf(5), alias, attempt, point, sha(point), c.canonicalSigningBytes(),
            byteArrayOf(), pending, byteArrayOf(), byteArrayOf(1), bytes(0x50), byteArrayOf())
        val phases = ArrayList<Int>(); var closes = 0; var changedScope = false; var changedReceipt = false
        var credentialIntakes = 0; var credential = byteArrayOf()
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int; phases.add(phase)
            return if (method == 21) when (phase) {
                12 -> reserved.map(ByteArray::copyOf).toTypedArray()
                13 -> prepared.map(ByteArray::copyOf).toTypedArray()
                14 -> arrayOf(policyOriginal.copyOf())
                7 -> arrayOf(byteArrayOf(5), alias.copyOf(), point.copyOf(), sha(raw), KagemushaCoreCoordinatorFrameV1.u32(raw.size),
                    ByteArray(314) { 0x39 }, if (changedScope) bytes(0x41) else pending.copyOf())
                8 -> arrayOf(prepared[7], attempt)
                10 -> arrayOf(fields[2], raw.copyOf(), sha(raw), KagemushaCoreCoordinatorFrameV1.u32(raw.size))
                else -> error("Fixture cannot invoke identity platform work or money")
            } else {
                check(method == 20)
                when (phase) {
                    1 -> eFields.map(ByteArray::copyOf).toTypedArray()
                    5 -> arrayOf(byteArrayOf(2), der.copyOf(), receipt())
                    6 -> arrayOf(pending.copyOf(), sha(eMessage))
                    8 -> { credentialIntakes++; credential = fields[2].copyOf(); arrayOf(sha(credential), pending.copyOf()) }
                    else -> error("Fixture cannot invoke possession platform work or financial admission")
                }
            }
        }
        private fun receipt(): ByteArray = "KGMAPP1\u0000".toByteArray(Charsets.US_ASCII) + byteArrayOf(1, 0, 2) + le64(8) +
            (if (changedReceipt) bytes(0x42) else attempt) + pending + sha(eMessage) + sha(der) + pending + ByteArray(5)
    }

    companion object {
        private val der = byteArrayOf(0x30, 6, 2, 1, 1, 2, 1, 1)
        private fun bytes(n: Int) = ByteArray(32) { n.toByte() }
        private fun le64(n: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(n).array()
        private fun sha(b: ByteArray) = MessageDigest.getInstance("SHA-256").digest(b)
        private fun hex(b: ByteArray) = b.joinToString("") { "%02x".format(it.toInt() and 255) }
        private fun base64(b: ByteArray) = Base64.getEncoder().encodeToString(b)
        private fun json(f: Map<String, Any?>) = JsonEncoder.encode(f).toByteArray(Charsets.UTF_8)
        @Suppress("UNCHECKED_CAST") private fun fields(b: ByteArray) = JsonParser.parse(b.toString(Charsets.UTF_8)) as Map<String, Any?>
        private fun framed(domain: String, body: ByteArray) = (domain + '\u0000').toByteArray(Charsets.US_ASCII) + le64(body.size.toLong()) + body
        private fun token(e: Endpoint, project: Long = 7, opaque: String = "original-opaque-google-token") =
            KagemushaAndroidPlayIntegrityTokenOriginalV1(project, e.c.playIntegrityRequestHash(sha(e.point)), opaque)
        private fun policy(number: Long = 7) = json(linkedMapOf("schema" to "iroha.kagemusha.play-integrity-verification-policy.v1",
            "version" to 1L, "cloudProject" to linkedMapOf("id" to "fixture-project", "number" to number),
            "packageName" to "pg.bpng.digitalkina", "packageVersion" to 1L,
            "appSigningCertificateSha256Hex" to "7".repeat(64), "credentialSubject" to linkedMapOf(
                "email" to "operator@fixture-project.iam.gserviceaccount.com", "clientId" to "123")))
    }
}
