// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Model-owned public HTTP encoding and correlation only. These bytes grant no native authority. */
object KagemushaOrdinaryIdentityHttpCodecV1 {
    fun preparationBody(fields: List<ByteArray>): ByteArray {
        requireReservation(fields)
        val body = linkedMapOf<String, Any?>("account_id" to text(fields[1]),
            "client_nonce_hex" to hex(fields[2]), "release_id_hex" to hex(fields[3]),
            "profile_id_hex" to hex(fields[4]), "lane_id_hex" to hex(fields[5]),
            "financial_authority_commitment_hex" to hex(fields[6]))
        return JsonEncoder.encode(body).toByteArray(Charsets.UTF_8)
    }
    fun rawAttestationBody(signedC: ByteArray, point: ByteArray, raw: ByteArray): ByteArray {
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
        val challenge = c.attestationChallenge()
        require(point.size == 65 && point[0] == 4.toByte() && raw.size in 1..131072)
        return JsonEncoder.encode(linkedMapOf(
            "schema" to "iroha.kagemusha.ordinary-app-raw-admission-request.v1", "operation" to "issue",
            "operation_id" to hex(challenge), "signed_preparation_base64" to base64(signedC),
            "attested_public_key_sec1_base64" to base64(point), "raw_attestation_base64" to base64(raw))).toByteArray(Charsets.UTF_8)
    }
    fun signedPreparationResponse(raw: ByteArray, reservation: List<ByteArray>): ByteArray {
        requireReservation(reservation)
        val fields = objectFields(raw)
        exact(fields, "operation_id", "signed_preparation_base64", "attestation_challenge_base64", "expires_at_ms")
        val signed = unbase64(string(fields, "signed_preparation_base64"), 515)
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signed)
        same(c.clientNonce(), reservation[2]); same(c.releaseId(), reservation[3])
        same(c.hardwareProfileId(), reservation[4]); same(c.laneId(), reservation[5])
        same(c.financialAuthorityCommitment(), reservation[6])
        require(string(fields, "operation_id") == hex(c.attestationChallenge()))
        same(unbase64(string(fields, "attestation_challenge_base64"), 32), c.attestationChallenge())
        require(unsigned(fields.getValue("expires_at_ms")) == c.expiresAtMs)
        return signed
    }
    fun rawAdmissionResponse(raw: ByteArray): ByteArray {
        val fields = objectFields(raw); exact(fields, "raw_admission_base64", "raw_admission_sha256_hex")
        val signed = unbase64(string(fields, "raw_admission_base64"), 314)
        require(string(fields, "raw_admission_sha256_hex") == hex(sha(signed)))
        return signed
    }
    /** Pure carrier encoding; only a private paired Native holder can select these inputs for transport. */
    fun certificateBody(signedC: ByteArray, point: ByteArray, raw: ByteArray, possessionDer: ByteArray,
        opaqueIntegrityToken: String?): ByteArray {
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
        require(point.size == 65 && point[0] == 4.toByte() && raw.size in 1..131072 && possessionDer.size in 8..72)
        require(opaqueIntegrityToken == null || opaqueIntegrityToken.isNotEmpty() && opaqueIntegrityToken.length <= 64 * 1024 &&
            opaqueIntegrityToken.all { it.code in 0x21..0x7e })
        return JsonEncoder.encode(linkedMapOf("schema" to "iroha.kagemusha.ordinary-app-credential-request.v1",
            "operation" to "issue", "operation_id" to hex(c.attestationChallenge()),
            "signed_preparation_base64" to base64(signedC), "attested_public_key_sec1_base64" to base64(point),
            "raw_attestation_base64" to base64(raw), "app_possession" to linkedMapOf("platform" to "android_keystore",
                "signature_der_base64" to base64(possessionDer)), "play_integrity_token" to opaqueIntegrityToken))
            .toByteArray(Charsets.UTF_8).also { require(it.size <= 256 * 1024) }
    }
    fun certificateResponse(raw: ByteArray): ByteArray {
        val fields = objectFields(raw); exact(fields, "certificate_base64", "certificate_sha256_hex")
        val encoded = string(fields, "certificate_base64")
        require(encoded.length <= ((16 * 1024 + 2) / 3) * 4)
        val original = Base64.getDecoder().decode(encoded)
        require(original.size in 1..16 * 1024 && base64(original) == encoded)
        require(string(fields, "certificate_sha256_hex") == hex(sha(original)))
        return original
    }
    /** Public data projection of the sole governed Google policy original; parsing grants no policy admission. */
    fun playIntegrityCloudProjectOriginal(raw: ByteArray): Long {
        require(raw.size in 1..16 * 1024)
        val fields = objectFields(raw)
        exact(fields, "schema", "version", "cloudProject", "packageName", "packageVersion",
            "appSigningCertificateSha256Hex", "credentialSubject")
        require(string(fields, "schema") == "iroha.kagemusha.play-integrity-verification-policy.v1" &&
            unsigned(fields.getValue("version")) == BigInteger.ONE)
        @Suppress("UNCHECKED_CAST") val project = fields["cloudProject"] as? Map<String, Any?> ?: error("Missing selected Google project")
        exact(project, "id", "number")
        val id = string(project, "id")
        require(id.matches(Regex("[a-z][a-z0-9-]{4,28}[a-z0-9]")))
        val number = unsigned(project.getValue("number"))
        require(number.bitLength() <= 63) { "Selected Google project exceeds the Android SDK numeric range" }
        @Suppress("UNCHECKED_CAST") val subject = fields["credentialSubject"] as? Map<String, Any?> ?: error("Missing selected Google principal")
        exact(subject, "email", "clientId")
        require(string(subject, "email").matches(Regex("[a-z][a-z0-9-]{4,28}[a-z0-9]@" + Regex.escape(id) + "\\.iam\\.gserviceaccount\\.com")))
        require(string(subject, "clientId").matches(Regex("[1-9][0-9]{0,19}")))
        require(string(fields, "packageName").length <= 255 &&
            string(fields, "packageName").matches(Regex("[A-Za-z_][A-Za-z0-9_]*(?:\\.[A-Za-z_][A-Za-z0-9_]*)+")))
        unsigned(fields.getValue("packageVersion"))
        require(string(fields, "appSigningCertificateSha256Hex").matches(Regex("[0-9a-f]{64}")))
        return number.toLong()
    }
    fun certificateRequestId(attempt: ByteArray): String {
        require(attempt.size == 32 && attempt.any { it != 0.toByte() })
        return rawAttestationRequestId(sha("iroha:kagemusha:v1:ordinary-app-certificate-http\u0000".toByteArray(Charsets.US_ASCII) + attempt))
    }
    /** Strict complete Native body correlation. This never constructs missing originals. */
    fun requireRetailStartOriginal(raw: ByteArray, signedC: ByteArray, credential: ByteArray): ByteArray {
        KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
        require(credential.size in 1..16 * 1024)
        val fields = objectFields(raw)
        exact(fields, "wallet", "signed_preparation_base64", "raw_admission_original_base64",
            "platform_original_base64", "core_possession_original_base64", "app_certificate_base64", "selected_integrity")
        val wallet = string(fields, "wallet")
        require(wallet.toByteArray(Charsets.UTF_8).size in 1..4096)
        // Canonical account/C binding is checked by the genuine Model-backed Native exporter.
        // This detached DATA checker retains text and grants no admitted owner or account.
        require(wallet.all { it.code in 0x21..0x7e })
        same(unbase64(string(fields, "signed_preparation_base64"), 515), signedC)
        unbase64(string(fields, "raw_admission_original_base64"), 314)
        boundedBase64(string(fields, "platform_original_base64"), 128 * 1024)
        boundedBase64(string(fields, "core_possession_original_base64"), 5120)
        same(boundedBase64(string(fields, "app_certificate_base64"), 16 * 1024), credential)
        require(fields["selected_integrity"] == null)
        return raw.copyOf()
    }
    /** Assemble only complete Native phase15 chunks. No caller field or digest recreates an original. */
    fun retailStartOriginalChunks(chunks: List<List<ByteArray>>, signedC: ByteArray,
        credential: ByteArray, scope: ByteArray, credentialDigest: ByteArray, nativeTicket: ByteArray): ByteArray {
        require(chunks.size in 1..4 && scope.size == 32 && credentialDigest.size == 32)
        val first = chunks.first()
        chunks.forEachIndexed { index, fields ->
            val request = listOf(KagemushaCoreCoordinatorFrameV1.u32(15), nativeTicket.copyOf(),
                KagemushaCoreCoordinatorFrameV1.u32(index))
            KagemushaAppOwnedHardwareFrameV1.requireRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, request)
            KagemushaAppOwnedHardwareFrameV1.requireResponse(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, request, fields)
            same(fields[2], first[2]); same(fields[3], first[3]); same(fields[4], scope); same(fields[5], credentialDigest)
        }
        val total = ByteBuffer.wrap(first[3]).order(java.nio.ByteOrder.LITTLE_ENDIAN).int
        require(chunks.size == (total + 65535) / 65536)
        val body = ByteArray(total)
        chunks.forEachIndexed { index, fields -> fields[1].copyInto(body, index * 65536) }
        same(sha(body), first[2])
        return requireRetailStartOriginal(body, signedC, credential)
    }
    /** Public response correlation only; Native phase9 separately admits the full FI challenge. */
    fun retailStartResponse(raw: ByteArray, signedC: ByteArray): List<ByteArray> {
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
        val fields = objectFields(raw)
        exact(fields, "challenge_id", "canonical_challenge_base64", "account_signing_message_base64", "expires_at_ms")
        require(string(fields, "challenge_id") == hex(c.attestationChallenge()))
        val expires = unsigned(fields.getValue("expires_at_ms"))
        require(expires > c.issuedAtMs && expires <= c.expiresAtMs)
        return listOf(boundedBase64(string(fields, "canonical_challenge_base64"), 32 * 1024),
            unbase64(string(fields, "account_signing_message_base64"), 32))
    }
    fun retailFinishBody(attempt: ByteArray, walletSignature: ByteArray): ByteArray {
        require(attempt.size == 32 && attempt.any { it != 0.toByte() } && walletSignature.size == 64)
        return JsonEncoder.encode(linkedMapOf("challenge_id" to hex(attempt),
            "account_signature_base64" to base64(walletSignature))).toByteArray(Charsets.UTF_8)
    }
    /** Detached FI original and claimed ID; Native phase12 admits and independently derives the ID. */
    fun retailFinishResponse(raw: ByteArray, attempt: ByteArray): List<ByteArray> {
        require(attempt.size == 32 && attempt.any { it != 0.toByte() })
        val fields = objectFields(raw); exact(fields, "challenge_id", "enrollment_id_hex", "canonical_certificate_base64")
        require(string(fields, "challenge_id") == hex(attempt))
        val id = string(fields, "enrollment_id_hex")
        require(id.matches(Regex("[0-9a-f]{64}")) && id.any { it != '0' })
        return listOf(boundedBase64(string(fields, "canonical_certificate_base64"), 16 * 1024),
            id.chunked(2).map { it.toInt(16).toByte() }.toByteArray())
    }
    fun retailRequestId(attempt: ByteArray, kind: String): String {
        require(kind == "start" || kind == "finish")
        require(attempt.size == 32 && attempt.any { it != 0.toByte() })
        return rawAttestationRequestId(sha(("iroha:kagemusha:v1:ordinary-retail-$kind-http\u0000")
            .toByteArray(Charsets.US_ASCII) + attempt))
    }
    private fun boundedBase64(value: String, maximum: Int): ByteArray {
        require(value.length <= ((maximum + 2) / 3) * 4)
        return Base64.getDecoder().decode(value).also { require(it.size in 1..maximum && base64(it) == value) }
    }
    fun rawAttestationRequestId(hash: ByteArray): String {
        require(hash.size == 32 && hash.any { it != 0.toByte() })
        val uuid = hash.copyOfRange(0, 16)
        uuid[6] = ((uuid[6].toInt() and 15) or 64).toByte(); uuid[8] = ((uuid[8].toInt() and 63) or 128).toByte()
        val h = hex(uuid)
        return "${h.substring(0, 8)}-${h.substring(8, 12)}-${h.substring(12, 16)}-${h.substring(16, 20)}-${h.substring(20)}"
    }
    private fun requireReservation(fields: List<ByteArray>) = KagemushaOrdinaryAppIdentityFrameV1.requireResponse(
        listOf(KagemushaCoreCoordinatorFrameV1.u32(12)), fields)
    @Suppress("UNCHECKED_CAST") private fun objectFields(raw: ByteArray): Map<String, Any?> {
        require(raw.size in 1..256 * 1024)
        return JsonParser.parse(text(raw)) as? Map<String, Any?> ?: error("Ordinary response is not an object")
    }
    private fun text(bytes: ByteArray): String = Charsets.UTF_8.newDecoder()
        .onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
        .decode(ByteBuffer.wrap(bytes)).toString()
    private fun exact(fields: Map<String, Any?>, vararg keys: String) = require(fields.keys == keys.toSet())
    private fun string(fields: Map<String, Any?>, key: String) = fields.getValue(key) as? String ?: error("Ordinary field is not text")
    private fun unsigned(value: Any?): BigInteger = when (value) {
        is Long -> BigInteger.valueOf(value)
        is BigInteger -> value
        else -> error("Ordinary interval is not an integer")
    }.also { require(it.signum() > 0 && it.bitLength() <= 64) }
    private fun unbase64(value: String, width: Int): ByteArray {
        require(value.length <= ((width + 2) / 3) * 4)
        return Base64.getDecoder().decode(value).also { require(it.size == width && base64(it) == value) }
    }
    private fun base64(raw: ByteArray) = Base64.getEncoder().encodeToString(raw)
    private fun hex(raw: ByteArray) = raw.joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
    private fun same(a: ByteArray, b: ByteArray) = require(MessageDigest.isEqual(a, b)) { "Ordinary public reply substituted the native reservation" }
}
