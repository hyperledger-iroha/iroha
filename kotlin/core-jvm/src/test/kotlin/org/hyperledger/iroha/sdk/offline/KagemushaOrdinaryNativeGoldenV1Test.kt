// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.security.AlgorithmParameters
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.Signature
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.security.spec.ECPoint
import java.security.spec.ECPublicKeySpec
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Actual Native-produced byte parity and known-public signature diagnostics only.
 * The fixture's attestation and Google token are inert. No holder, policy admission,
 * physical device qualification, enrollment or monetary permission is established here.
 */
class KagemushaOrdinaryNativeGoldenV1Test {
    @Test fun originalNativePreparationsAndEveryAttemptEquationMatchBothPlatforms() {
        for (v in vectors()) {
            val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(v.bytes("signed_preparation_base64"))
            assertContentEquals(v.bytes("preparation_signing_message_base64"), c.canonicalSigningBytes())
            assertEquals(v.getValue("attestation_challenge_hex"), hex(c.attestationChallenge()))
            assertEquals(v.getValue("operation_id"), hex(c.attestationChallenge()))
            assertEquals(v.getValue("attested_key_id_hex"), hex(sha(v.bytes("app_public_key_sec1_base64"))))
            assertEquals(v.getValue("play_integrity_request_hash_hex"), hex(c.playIntegrityRequestHash(unhex(v.text("attested_key_id_hex")))))
            assertFalse(c.attestationChallenge().contentEquals(c.enrollmentId()))
            assertFalse(c.attestationChallenge().contentEquals(sha(c.attestationChallenge())))
            assertFalse(c.attestationChallenge().contentEquals(sha(c.transportBytes())))
            val e = v.bytes("enrollment_possession_signing_message_base64")
            val body = e.copyOfRange(e.size - 371, e.size)
            assertContentEquals(c.attestationChallenge(), body.copyOfRange(3, 35))
            assertContentEquals(unhex(v.text("raw_attestation_sha256_hex")), body.copyOfRange(323, 355))
            assertContentEquals(sha(v.bytes("raw_attestation_base64")), body.copyOfRange(323, 355))
            assertContentEquals(c.transportBytes().copyOfRange(435, 451), body.copyOfRange(355, 371))
        }
    }

    @Test fun nativeEFrameRefusesStableEnrollmentIdDoubleHashAndChangedSignedSelections() {
        for (v in vectors()) {
            val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(v.bytes("signed_preparation_base64"))
            val fields = eFields(v, c.canonicalSigningBytes())
            fun check(id: ByteArray, offered: List<ByteArray>) = KagemushaAppOwnedHardwareFrameV1.requireResponse(
                KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION,
                listOf(le32(1), id), offered)
            check(c.attestationChallenge(), fields)
            for (replacement in listOf(c.enrollmentId(), sha(c.attestationChallenge()))) {
                val changed = fields.map { it.copyOf() }.toMutableList()
                replacement.copyInto(changed[1], changed[1].size - 371 + 3)
                assertFails { check(replacement, changed) }
            }
            // Change each actual C selection while retaining the original E. A new alias/hash
            // cannot turn the prior platform signature into approval of another preparation.
            for (offset in listOf(427, 3 + 8 * 32, 3 + 9 * 32, 3 + 11 * 32)) {
                val changedC = c.transportBytes().also { it[offset] = (it[offset].toInt() xor 2).toByte() }
                val other = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(changedC)
                val changed = eFields(v, other.canonicalSigningBytes())
                assertFails { check(other.attestationChallenge(), changed) }
            }
        }
    }

    @Test fun originalAndroidPossessionAndRefreshSignaturesVerifyExactNativeMessages() {
        val v = vectors().single { it.text("platform") == "android_keymint" }
        val pop = v.objectValue("certificate_request").objectValue("app_possession").bytes("signature_der_base64")
        val message = v.bytes("enrollment_possession_signing_message_base64")
        val point = v.bytes("app_public_key_sec1_base64")
        assertTrue(verify(point, message, pop))
        assertFalse(verify(point, message.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }, pop))
        val r = v.objectValue("integrity_refresh")
        val refresh = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(r.bytes("signed_refresh_challenge_base64"))
        assertContentEquals(r.bytes("refresh_signing_message_base64"), refresh.canonicalSigningBytes())
        assertContentEquals(r.bytes("possession_signing_message_base64"), refresh.possessionSigningBytes())
        assertEquals(r.getValue("operation_id"), hex(refresh.operationId()))
        assertEquals(r.getValue("play_integrity_request_hash_hex"), hex(refresh.playIntegrityRequestHash()))
        assertTrue(verify(point, refresh.possessionSigningBytes(), r.bytes("possession_der_base64")))
        assertFalse(verify(point, refresh.canonicalSigningBytes(), r.bytes("possession_der_base64")))
    }

    @Test fun mandatoryIssuerCountersignatureBindsFullEdOriginalAndIsDistinctFromAppKey() {
        for (v in vectors()) {
            issuer(v, "ordinary_credential", 1)
            if (v["integrity_refresh"] != null) issuer(v.objectValue("integrity_refresh"), "lease", 2,
                v.bytes("ordinary_issuer_public_key_sec1_base64"), v.bytes("app_public_key_sec1_base64"))
        }
    }

    @Test fun exactOpaqueNativeHttpOriginalsSurviveClosedCorrelationWithoutLegacySizeAssumption() {
        for (v in vectors()) {
            val original = v.bytes("ordinary_credential_base64")
            assertContentEquals(original, KagemushaOrdinaryIdentityHttpCodecV1.certificateResponse(json(v.objectValue("certificate_response"))))
            assertContentEquals(v.objectValue("raw_response").bytes("raw_admission_base64"),
                KagemushaOrdinaryIdentityHttpCodecV1.rawAdmissionResponse(json(v.objectValue("raw_response"))))
            assertTrue(original.size in 1..16 * 1024)
            assertFalse(original.contentEquals(v.bytes("ordinary_credential_ed_original_base64")))
            val altered = v.objectValue("certificate_response") + ("certificate_sha256_hex" to "0".repeat(64))
            assertFails { KagemushaOrdinaryIdentityHttpCodecV1.certificateResponse(json(altered)) }
            assertFails { KagemushaOrdinaryIdentityHttpCodecV1.certificateResponse(json(v.objectValue("certificate_response") + ("qualified" to true))) }
            if (v.text("platform") == "android_keymint") {
                val request = v.objectValue("certificate_request")
                val generated = KagemushaOrdinaryIdentityHttpCodecV1.certificateBody(v.bytes("signed_preparation_base64"),
                    v.bytes("app_public_key_sec1_base64"), v.bytes("raw_attestation_base64"),
                    request.objectValue("app_possession").bytes("signature_der_base64"), request.text("play_integrity_token"))
                assertEquals(request, objectFields(generated))
            }
        }
    }

    private fun issuer(v: Map<String, Any?>, prefix: String, purpose: Int,
        point: ByteArray = v.bytes("ordinary_issuer_public_key_sec1_base64"),
        appPoint: ByteArray = v.bytes("app_public_key_sec1_base64")) {
        val admission = v.bytes("${prefix}_issuer_admission_base64")
        assertEquals(163, admission.size); assertEquals(purpose, admission[2].toInt())
        val ed = v.bytes("${prefix}_ed_original_base64")
        assertContentEquals(sha(ed), admission.copyOfRange(67, 99))
        val message = "iroha:kagemusha:v1:ordinary-issuer-circuit-admission\u0000".toByteArray() + le64(99) + admission.copyOf(99)
        val der = org.bouncycastle.asn1.DERSequence(arrayOf(
            org.bouncycastle.asn1.ASN1Integer(BigInteger(1, admission.copyOfRange(99, 131))),
            org.bouncycastle.asn1.ASN1Integer(BigInteger(1, admission.copyOfRange(131, 163))))).encoded
        assertTrue(verify(point, message, der)); assertFalse(verify(appPoint, message, der))
        val changed = message.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertFalse(verify(point, changed, der))
    }
    private fun eFields(v: Map<String, Any?>, fullC: ByteArray): List<ByteArray> {
        val android = v.text("platform") == "android_keymint"
        val point = v.bytes("app_public_key_sec1_base64"); val key = sha(point)
        val alias = if (android) KagemushaOrdinaryAppKeyAliasV1.originalAlias(fullC) else Base64.getEncoder().encodeToString(key)
        return listOf(le64(1), v.bytes("enrollment_possession_signing_message_base64"), byteArrayOf(if (android) 5 else 4),
            alias.toByteArray(), sha(fullC), point, key, fullC, byteArrayOf(), sha(v.bytes("raw_attestation_base64")),
            if (android) byteArrayOf() else le32(1), byteArrayOf(if (android) 2 else 0),
            sha(v.objectValue("raw_response").bytes("raw_admission_base64")), byteArrayOf())
    }
    private fun verify(point: ByteArray, message: ByteArray, der: ByteArray): Boolean {
        val parameters = AlgorithmParameters.getInstance("EC").also { it.init(ECGenParameterSpec("secp256r1")) }
            .getParameterSpec(ECParameterSpec::class.java)
        val key = KeyFactory.getInstance("EC").generatePublic(ECPublicKeySpec(ECPoint(
            BigInteger(1, point.copyOfRange(1, 33)), BigInteger(1, point.copyOfRange(33, 65))), parameters))
        return Signature.getInstance("SHA256withECDSA").run { initVerify(key); update(message); verify(der) }
    }
    private fun vectors(): List<Map<String, Any?>> {
        val path = generateSequence(Paths.get(System.getProperty("user.dir")).toAbsolutePath()) { it.parent }
            .map { it.resolve("fixtures/offline/kagemusha_ordinary_app_enrollment_v1.json") }.first { Files.isRegularFile(it) }
        val raw = Files.readAllBytes(path)
        assertEquals("5268e97fa1f54f9c23208efc5ee0a39f6c6284e122bf31ff781cb54ce3b93d96", hex(sha(raw)))
        val f = objectFields(raw); assertEquals(false, f["authority"])
        assertEquals("iroha.kagemusha.ordinary-app-enrollment-public-codec-fixture.v1", f["schema"])
        @Suppress("UNCHECKED_CAST") return (f.getValue("vectors") as List<Map<String, Any?>>).also { assertEquals(2, it.size) }
    }
    @Suppress("UNCHECKED_CAST") private fun objectFields(raw: ByteArray) = JsonParser.parse(raw.toString(Charsets.UTF_8)) as Map<String, Any?>
    @Suppress("UNCHECKED_CAST") private fun Map<String, Any?>.objectValue(k: String) = getValue(k) as Map<String, Any?>
    private fun Map<String, Any?>.text(k: String) = getValue(k) as String
    private fun Map<String, Any?>.bytes(k: String) = Base64.getDecoder().decode(text(k))
    private fun json(v: Map<String, Any?>) = JsonEncoder.encode(v).toByteArray()
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
    private fun hex(raw: ByteArray) = raw.joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun unhex(v: String) = v.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun le64(v: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(v).array()
    private fun le32(v: Int) = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(v).array()
}
