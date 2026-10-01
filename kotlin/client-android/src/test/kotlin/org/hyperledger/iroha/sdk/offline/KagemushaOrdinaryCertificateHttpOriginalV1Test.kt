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

    @Test fun fiStartSelectsTheSameAdmittedCredentialAndNativeRetainsItsFullChallenge() = runBlocking {
        val e = Endpoint(); val held = Held(e); val credential = byteArrayOf(0x71, 0x72)
        held.e.acceptOriginalCredential(credential)
        val request = held.e.retailStartRequestOriginal(held.reservation, held.identity)
        assertEquals("/v1/offline/enrollment/ordinary/start", request.path)
        assertEquals(mapOf("signed_preparation_base64" to base64(e.signedC),
            "app_certificate_base64" to base64(credential)), fields(request.body()))
        val retail = held.e.prepareOriginalRetailEnrollment(held.reservation, held.identity) { original ->
            assertContentEquals(request.body(), original.body()); e.retailStartReply()
        }
        assertEquals(1, e.retailIntakes)
        assertContentEquals(e.retailMessage, retail.accountSigningBytes())
        assertNull(retail.recoverOriginalAccountSignature())
    }

    @Test fun fiStartCannotDispatchNativeIntakeAfterCurrentOwnerChangeOrWithExpiredPublicReply() = runBlocking {
        for (replaceOwner in listOf(true, false)) {
            val e = Endpoint(); val held = Held(e); held.e.acceptOriginalCredential(byteArrayOf(0x71))
            assertFails { held.e.prepareOriginalRetailEnrollment(held.reservation, held.identity) {
                if (replaceOwner) { e.changedScope = true; e.retailStartReply() }
                else json(fields(e.retailStartReply()).toMutableMap().also { it["expires_at_ms"] = 121001L })
            } }
            assertEquals(0, e.retailIntakes)
        }
    }

    @Test fun sharedWorkflowCompletesOriginalsAndRepeatedSetupDoesNotSignOrRequestAgain() = runBlocking {
        val e = Endpoint(); var walletCalls = 0; var httpCalls = 0
        val workflow = e.workflow({ request -> httpCalls++; e.reply(request) }, {
            walletCalls++; assertContentEquals(e.retailMessage, it); ByteArray(64) { 0x61 }
        })
        val first = workflow.beginOrResume()
        assertContentEquals(e.enrollmentId, first.enrollmentId())
        assertContentEquals(e.retailCertificate, first.originalRetailCertificate())
        assertEquals(1, walletCalls); assertEquals(1, e.retailIntakes); assertEquals(1, e.retailCompletions)
        assertEquals(4, httpCalls) // C, credential, FI start, FI finish; raw is already retained by the fixture.
        first.enrollmentId().fill(0); first.originalRetailCertificate().fill(0)
        val second = workflow.beginOrResume()
        assertContentEquals(e.enrollmentId, second.enrollmentId())
        assertEquals(1, walletCalls); assertEquals(4, httpCalls)
    }

    @Test fun sharedWorkflowKeepsTheWalletOriginalAcrossAnAmbiguousHttpFinish() = runBlocking {
        val e = Endpoint(); var walletCalls = 0; var failFinish = true; val requests = ArrayList<Pair<String, ByteArray>>()
        val workflow = e.workflow({ request ->
            if (request.path.endsWith("/finish")) {
                requests.add(request.requestId to request.body())
                if (failFinish) { failFinish = false; error("Protected transport lost the reply") }
            }
            e.reply(request)
        }, { walletCalls++; ByteArray(64) { 0x61 } })
        assertFails { workflow.beginOrResume() }
        assertEquals(1, walletCalls); assertEquals(0, e.retailCompletions)
        val completed = workflow.beginOrResume()
        assertContentEquals(e.enrollmentId, completed.enrollmentId()); assertEquals(1, walletCalls)
        assertEquals(2, requests.size); assertEquals(requests[0].first, requests[1].first)
        assertContentEquals(requests[0].second, requests[1].second)
    }

    @Test fun sharedWorkflowRetainsOneGoogleOriginalAcrossIssuerAndFiRetry() = runBlocking {
        val e = Endpoint(policyOriginal = policy()); var googleCalls = 0; var failFinish = true; val tokens = ArrayList<Any?>()
        val backend = object : KagemushaPlayIntegrityBackendV1 {
            override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> {
                assertEquals(7L, cloudProjectNumber)
                return CompletableFuture.completedFuture(object : KagemushaPlayIntegrityPreparedV1 {
                    override fun request(originalHashText: String): CompletableFuture<String> {
                        googleCalls++
                        assertEquals(Base64.getUrlEncoder().withoutPadding().encodeToString(e.c.playIntegrityRequestHash(sha(e.point))), originalHashText)
                        return CompletableFuture.completedFuture("retained-original-google-token")
                    }
                })
            }
        }
        val workflow = e.workflow({ request ->
            if (request.path.endsWith("/certificate")) tokens.add(fields(request.body())["play_integrity_token"])
            if (request.path.endsWith("/finish") && failFinish) { failFinish = false; error("Lost protected reply") }
            e.reply(request)
        }, { ByteArray(64) { 0x61 } }, backend)
        assertFails { workflow.beginOrResume() }
        assertContentEquals(e.enrollmentId, workflow.beginOrResume().enrollmentId())
        assertEquals(1, googleCalls); assertEquals(listOf<Any?>("retained-original-google-token", "retained-original-google-token"), tokens)
    }

    @Test fun sharedWorkflowRetriesOnlyAnExplicitInvalidGoogleProviderOnTheNextUserAction() = runBlocking {
        val e = Endpoint(policyOriginal = policy()); var googleCalls = 0; var httpCertificates = 0
        val invalid = IllegalStateException("fixture explicit invalid-provider code")
        val backend = object : KagemushaPlayIntegrityBackendV1 {
            override fun invalidatesPreparedProvider(error: Throwable) = error === invalid
            override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> =
                CompletableFuture.completedFuture(object : KagemushaPlayIntegrityPreparedV1 {
                override fun request(originalHashText: String): CompletableFuture<String> {
                    googleCalls++
                    return if (googleCalls == 1) CompletableFuture<String>().also { it.completeExceptionally(invalid) }
                    else CompletableFuture.completedFuture("retained-original-google-token")
                }
            })
        }
        val workflow = e.workflow({ request ->
            if (request.path.endsWith("/certificate")) httpCertificates++
            e.reply(request)
        }, { ByteArray(64) { 0x61 } }, backend)
        assertFails { workflow.beginOrResume() }; assertEquals(1, googleCalls); assertEquals(0, httpCertificates)
        assertContentEquals(e.enrollmentId, workflow.beginOrResume().enrollmentId())
        assertEquals(2, googleCalls); assertEquals(1, httpCertificates)
    }

    @Test fun sharedWorkflowBootstrapsOnlyTheCompletedFiOriginalAndRetainsOneHardwareInvocation() = runBlocking {
        val e = Endpoint(); var walletCalls = 0; var httpCalls = 0; var osCalls = 0
        val workflow = e.workflow({ request -> httpCalls++; e.reply(request) }, { walletCalls++; ByteArray(64) { 0x61 } },
            bootstrapSigner = { prepared -> prepared.performPlatformSigning { alias, challenge, point, digest, message, _, guard ->
                osCalls++; assertEquals(3, e.retailState); assertEquals(1, e.bootstrapState); guard()
                assertEquals(e.alias.toString(Charsets.UTF_8), alias); assertContentEquals(e.attempt, challenge)
                assertContentEquals(e.point, point); assertContentEquals(sha(e.point), digest)
                assertContentEquals(e.bootstrapFields()[1], message); der.copyOf()
            } })
        val first = workflow.beginOrResumeBootstrapApproval()
        val expected = sha("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\u0000".toByteArray(Charsets.US_ASCII) + e.retailCertificate)
        assertContentEquals(expected, first.bootstrapOperationId())
        assertContentEquals(e.retailCertificate, first.originalRetailCertificate())
        assertContentEquals(e.enrollmentId, first.enrollmentId())
        assertContentEquals(e.bootstrapReceipt(), first.originalApprovalReceipt())
        assertContentEquals(der, e.bootstrapRaw)
        assertEquals(1, e.bootstrapPreparations); assertEquals(1, e.bootstrapRetains)
        first.bootstrapOperationId().fill(0); first.originalApprovalReceipt().fill(0); first.originalBootstrapSelection().fill(0)
        val again = workflow.beginOrResumeBootstrapApproval()
        assertContentEquals(expected, again.bootstrapOperationId()); assertContentEquals(e.bootstrapReceipt(), again.originalApprovalReceipt())
        assertEquals(4, httpCalls); assertEquals(1, walletCalls); assertEquals(1, osCalls)
        assertEquals(1, e.bootstrapPreparations); assertEquals(1, e.bootstrapRetains)
    }

    @Test fun sharedWorkflowCannotPrepareBootstrapBeforeAnAmbiguousFiFinishHasCompleted() = runBlocking {
        val e = Endpoint(); var lost = true; var osCalls = 0
        val workflow = e.workflow({ request ->
            if (request.path.endsWith("/finish") && lost) { lost = false; error("Lost FI reply") }; e.reply(request)
        }, { ByteArray(64) { 0x61 } }, bootstrapSigner = { prepared ->
            prepared.performPlatformSigning { _, _, _, _, _, _, guard -> osCalls++; guard(); der.copyOf() }
        })
        assertFails { workflow.beginOrResumeBootstrapApproval() }
        assertEquals(2, e.retailState); assertEquals(0, e.bootstrapPreparations); assertEquals(0, osCalls)
        val completed = workflow.beginOrResumeBootstrapApproval()
        assertContentEquals(e.bootstrapReceipt(), completed.originalApprovalReceipt())
        assertEquals(1, osCalls); assertEquals(1, e.bootstrapPreparations)
    }

    @Test fun sharedWorkflowRejectsACorrelatedAlternateBootstrapCredentialBeforeTheHardwareFence() = runBlocking {
        val e = Endpoint().apply { alternateBootstrapCredential = true }; var osCalls = 0
        val workflow = e.workflow(e::reply, { ByteArray(64) { 0x61 } }, bootstrapSigner = { prepared ->
            prepared.performPlatformSigning { _, _, _, _, _, _, _ -> osCalls++; der.copyOf() }
        })
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeBootstrapApproval() }
        assertEquals(3, e.retailState); assertEquals(0, e.bootstrapState); assertEquals(0, osCalls)
        assertTrue(e.closes > 0)
    }

    @Test fun sharedWorkflowBootstrapOwnerGuardStopsAChangedRetainedFiCertificate() = runBlocking {
        val e = Endpoint(); var osCalls = 0
        val workflow = e.workflow(e::reply, { ByteArray(64) { 0x61 } }, bootstrapSigner = { prepared ->
            prepared.performPlatformSigning { _, _, _, _, _, _, guard ->
                e.retailCertificate[0] = 0x66; guard(); osCalls++; der.copyOf()
            }
        })
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeBootstrapApproval() }
        assertEquals(1, e.bootstrapState); assertEquals(0, osCalls); assertEquals(0, e.bootstrapRetains)
        assertTrue(e.closes > 0)
        assertFails { workflow.beginOrResumeBootstrapApproval() }
        assertEquals(0, osCalls)
    }

    @Test fun sharedPublicationWorkflowReusesTheExactFiAndCapturedWWithoutHttpWalletOrOsRepeat() = runBlocking {
        val e = Endpoint().apply { publicationFixtureEnabled = true }
        var httpCalls = 0; var walletCalls = 0; var osCalls = 0
        val workflow = e.workflow({ request -> httpCalls++; e.reply(request) }, {
            walletCalls++; ByteArray(64) { 0x61 }
        }, bootstrapSigner = { prepared -> prepared.performPlatformSigning { _, _, _, _, _, _, guard ->
            osCalls++; guard(); der.copyOf()
        } })
        val first = workflow.beginOrResumeInitialStatePublication()
        assertContentEquals(e.enrollmentId, first.enrollmentId())
        assertContentEquals(sha(e.retailCertificate), first.retailCertificateOriginalDigest())
        assertContentEquals(sha(e.credential), first.appCredentialOriginalDigest())
        val captured = workflow.beginOrResumeBootstrapApproval()
        assertContentEquals(e.bootstrapReceipt(), captured.originalApprovalReceipt())
        first.publicationOriginalDigest().fill(0)
        val resumed = workflow.beginOrResumeInitialStatePublication()
        assertContentEquals(e.publicationFields()[2], resumed.publicationOriginalDigest())
        assertEquals(1, e.publicationCalls); assertEquals(1, e.publicationRecoveries)
        assertEquals(1, e.bootstrapPreparations); assertEquals(1, e.bootstrapRetains)
        assertEquals(1, e.retailCompletions); assertEquals(4, httpCalls)
        assertEquals(1, walletCalls); assertEquals(1, osCalls)
    }

    @Test fun lostNativePublicationReplyFreezesSharedWorkflowWithoutRepeatingAnyOriginalInvocation() = runBlocking {
        val e = Endpoint().apply { publicationFixtureEnabled = true; losePublicationReturn = true }
        var httpCalls = 0; var walletCalls = 0; var osCalls = 0
        val workflow = e.workflow({ request -> httpCalls++; e.reply(request) }, {
            walletCalls++; ByteArray(64) { 0x61 }
        }, bootstrapSigner = { prepared -> prepared.performPlatformSigning { _, _, _, _, _, _, guard ->
            osCalls++; guard(); der.copyOf()
        } })
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeInitialStatePublication() }
        assertTrue(e.publicationReplyRetained); assertEquals(1, e.closes)
        e.losePublicationReturn = false
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeInitialStatePublication() }
        assertEquals(1, e.publicationCalls); assertEquals(0, e.publicationRecoveries)
        assertEquals(4, httpCalls); assertEquals(1, walletCalls); assertEquals(1, osCalls)
        assertEquals(1, e.bootstrapPreparations); assertEquals(1, e.bootstrapRetains)
    }

    @Test fun missingAuthenticPublicationProviderRefusesSharedWorkflowAfterTheSameFiAndW() = runBlocking {
        val e = Endpoint(); var httpCalls = 0; var walletCalls = 0; var osCalls = 0
        val workflow = e.workflow({ request -> httpCalls++; e.reply(request) }, {
            walletCalls++; ByteArray(64) { 0x61 }
        }, bootstrapSigner = { prepared -> prepared.performPlatformSigning { _, _, _, _, _, _, guard ->
            osCalls++; guard(); der.copyOf()
        } })
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeInitialStatePublication() }
        assertEquals(3, e.retailState); assertEquals(3, e.bootstrapState)
        assertFalse(e.publicationReplyRetained); assertEquals(1, e.closes)
        assertFailsWith<IllegalStateException> { workflow.beginOrResumeInitialStatePublication() }
        assertEquals(1, e.publicationCalls); assertEquals(0, e.publicationRecoveries)
        assertEquals(4, httpCalls); assertEquals(1, walletCalls); assertEquals(1, osCalls)
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
        val retailMessage = bytes(0x60); val retailChallenge = byteArrayOf(0x51, 0x52); var retailIntakes = 0
        val retailCertificate = byteArrayOf(0x62, 0x63); val enrollmentId = bytes(0x64)
        var retailState = 0; var retailSignature = byteArrayOf(); var retailCompletions = 0
        var bootstrapState = 0; var bootstrapRaw = byteArrayOf(); var bootstrapPreparations = 0; var bootstrapRetains = 0
        var alternateBootstrapCredential = false
        var publicationFixtureEnabled = false; var publicationReplyRetained = false; var losePublicationReturn = false
        var publicationCalls = 0; var publicationRecoveries = 0
        // Inert transport commitments exercise workflow correlation only. No genuine Native
        // proof, financial current owner, release admission or device qualification is simulated.
        fun publicationFields() = listOf(le64(20), enrollmentId.copyOf(), bytes(0x81), sha(retailCertificate),
            sha(credential), bytes(0x82), bytes(0x83), bytes(0x84), bytes(0x85))
        private fun bootstrapOperationId() = sha("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\u0000".toByteArray(Charsets.US_ASCII) + retailCertificate)
        fun bootstrapFields(): List<ByteArray> {
            val credentialDigest = if (alternateBootstrapCredential) bytes(0x75) else sha(credential)
            val selection = framed("iroha:kagemusha:v1:hardware-transition-selection", byteArrayOf(1, 0) +
                listOf(c.releaseId(), bytes(0x61), bytes(0x62), credentialDigest, c.networkId(), c.laneId(), c.hardwareProfileId())
                    .flatMap { it.toList() }.toByteArray() + le64(1) + bytes(0x64) + le64(2) + byteArrayOf(0) + bytes(0x65) + ByteArray(96))
            val message = framed("iroha:kagemusha:v1:app-operation-approval", byteArrayOf(1, 0, 1) +
                listOf(bootstrapOperationId(), bytes(0x68), c.accountBinding(), c.appAuthorityPolicyDigest(), sha(point),
                    credentialDigest, sha(selection), bytes(0x69)).flatMap { it.toList() }.toByteArray() + le64(1000) + le64(121000))
            return listOf(le64(20), message, byteArrayOf(5), alias.copyOf(), attempt.copyOf(), point.copyOf(), sha(point),
                c.canonicalSigningBytes(), credentialDigest, bytes(0x70), byteArrayOf(), byteArrayOf(1), bytes(0x50), selection)
        }
        fun bootstrapReceipt(): ByteArray {
            val original = bootstrapFields()
            return "KGMAPP1\u0000".toByteArray(Charsets.US_ASCII) + byteArrayOf(1, 0, 1) + le64(20) +
                bootstrapOperationId() + original[9] + sha(original[1]) + sha(bootstrapRaw) + original[8] + ByteArray(5)
        }
        fun retailStartReply() = json(linkedMapOf("challenge_id" to hex(attempt),
            "canonical_challenge_base64" to base64(retailChallenge), "account_signing_message_base64" to base64(retailMessage),
            "expires_at_ms" to 121000L))
        fun reply(request: KagemushaOrdinaryIdentityHttpOriginalV1): ByteArray = when {
            request.path.endsWith("/prepare") -> json(linkedMapOf("operation_id" to hex(attempt),
                "signed_preparation_base64" to base64(signedC), "attestation_challenge_base64" to base64(attempt), "expires_at_ms" to 121000L))
            request.path.endsWith("/certificate") -> json(linkedMapOf("certificate_base64" to base64(byteArrayOf(0x71, 0x72)),
                "certificate_sha256_hex" to hex(sha(byteArrayOf(0x71, 0x72)))))
            request.path.endsWith("/start") -> retailStartReply()
            request.path.endsWith("/finish") -> json(linkedMapOf("challenge_id" to hex(attempt), "enrollment_id_hex" to hex(enrollmentId),
                "canonical_certificate_base64" to base64(retailCertificate)))
            else -> error("No alternate route")
        }
        fun workflow(transport: KagemushaOrdinaryIdentityOriginalTransportV1,
            signer: KagemushaOrdinaryWalletAccountSignerV1,
            backend: KagemushaPlayIntegrityBackendV1 = object : KagemushaPlayIntegrityBackendV1 {
                override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> = error("Absent Native PI policy")
            }, bootstrapSigner: ((KagemushaNativePreparedOrdinaryBootstrapApprovalV1) -> ByteArray)? = null): KagemushaAndroidOrdinaryEnrollmentV1 {
            val facade = KagemushaNativeAppApprovalCoordinatorV1(KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/ordinary-flow", this))
            return KagemushaAndroidOrdinaryEnrollmentV1(facade, transport, signer, { check(!changedScope) },
                { assertNotNull(it.recoverOriginalAttestation()) }, { assertNotNull(it.recoverOriginalPossession()) },
                KagemushaAndroidPlayIntegrityProviderV1(backend), bootstrapSigner)
        }
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int; phases.add(phase)
            if (method == 19) {
                check(retailState == 3) { "FI must complete before Bootstrap" }
                val original = bootstrapFields()
                if (phase != 8) assertContentEquals(le64(20), fields[1])
                return when (phase) {
                    8 -> { assertContentEquals(bootstrapOperationId(), fields[1]); bootstrapPreparations++; original.map(ByteArray::copyOf).toTypedArray() }
                    2 -> when (bootstrapState) {
                        0 -> { bootstrapState = 1; arrayOf(byteArrayOf(1), byteArrayOf(), byteArrayOf()) }
                        2 -> arrayOf(byteArrayOf(2), bootstrapRaw.copyOf(), byteArrayOf())
                        3 -> arrayOf(byteArrayOf(3), bootstrapRaw.copyOf(), bootstrapReceipt())
                        else -> error("Unknown hardware invocation")
                    }
                    3 -> { check(bootstrapState == 1); bootstrapRetains++; bootstrapRaw = fields[2].copyOf(); bootstrapState = 2; arrayOf(sha(bootstrapRaw)) }
                    4 -> { check(bootstrapState in 2..3); bootstrapState = 3; arrayOf(bootstrapReceipt()) }
                    5 -> when (bootstrapState) {
                        0 -> arrayOf(byteArrayOf(0), byteArrayOf(), byteArrayOf())
                        2 -> arrayOf(byteArrayOf(1), bootstrapRaw.copyOf(), byteArrayOf())
                        3 -> arrayOf(byteArrayOf(2), bootstrapRaw.copyOf(), bootstrapReceipt())
                        else -> error("Unknown hardware invocation")
                    }
                    6 -> arrayOf(original[9].copyOf(), sha(original[1]))
                    7 -> emptyArray()
                    9 -> {
                        check(bootstrapState == 3); publicationCalls++
                        check(publicationFixtureEnabled) { "Authentic initial publication provider is unavailable" }
                        publicationReplyRetained = true
                        check(!losePublicationReturn) { "Original publication reply lost after dispatch" }
                        publicationFields().map(ByteArray::copyOf).toTypedArray()
                    }
                    10 -> {
                        check(bootstrapState == 3); publicationRecoveries++
                        check(publicationFixtureEnabled && publicationReplyRetained)
                        publicationFields().map(ByteArray::copyOf).toTypedArray()
                    }
                    else -> error("No monetary fixture authority")
                }
            }
            return if (method == 21) when (phase) {
                12 -> reserved.map(ByteArray::copyOf).toTypedArray()
                11 -> arrayOf(attempt.copyOf())
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
                    9 -> {
                        assertContentEquals(retailChallenge, fields[2]); assertContentEquals(retailMessage, fields[3])
                        check(credential.isNotEmpty()); retailIntakes++
                        arrayOf(le64(19), retailChallenge.copyOf(), retailMessage.copyOf(), pending.copyOf(), sha(credential))
                    }
                    10 -> {
                        assertContentEquals(le64(19), fields[1])
                        if (retailState >= 2) arrayOf(byteArrayOf(2), retailSignature.copyOf())
                        else { check(retailState == 0); retailState = 1; arrayOf(byteArrayOf(1), byteArrayOf()) }
                    }
                    11 -> { check(retailState == 1); retailSignature = fields[2].copyOf(); retailState = 2; arrayOf(sha(retailSignature)) }
                    12 -> {
                        check(retailState in 2..3); assertContentEquals(retailCertificate, fields[2]); retailState = 3; retailCompletions++
                        arrayOf(enrollmentId.copyOf(), pending.copyOf())
                    }
                    13 -> { assertContentEquals(le64(19), fields[1]); arrayOf(byteArrayOf(retailState.toByte()), retailSignature.copyOf(),
                        if (retailState == 3) retailCertificate.copyOf() else byteArrayOf()) }
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
