// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets.UTF_8
import java.nio.file.Files
import java.nio.file.Paths
import java.util.Locale
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Public non-session fixture only. Canonical AccountAddress's real ABI bridge is mandatory.
 * These tests do not create a Native owner, refresh a lease, consume a nonce or contact a FI.
 */
class ParticipantEnrollmentHttpCodecV1Test {
    private val fixture: JsonObject = Json.parseToJsonElement(String(Files.readAllBytes(
        Paths.get("..", "..", "fixtures", "kagemusha", "participant_enrollment_http_v1.json")), UTF_8)).jsonObject
    private val vectors = fixture.getValue("vectors").jsonArray.map { it.jsonObject }
    private fun text(objectValue: JsonObject, name: String) = objectValue.getValue(name).jsonPrimitive.content
    private fun field(name: String) = text(fixture, name)
    private fun bytes(raw: String) = ByteArray(raw.length / 2) { raw.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    private fun operation(v: JsonObject) = when (text(v, "operation")) {
        "prepare" -> ParticipantEnrollmentHttpCodecV1.Operation.PREPARE
        "raw_attestation" -> ParticipantEnrollmentHttpCodecV1.Operation.RAW_ATTESTATION
        "certificate" -> ParticipantEnrollmentHttpCodecV1.Operation.CERTIFICATE
        else -> error("Unknown public fixture operation")
    }
    private fun context(v: JsonObject, namespace: String = field("authentication_namespace"),
        actor: String = field("actor_id"), target: URI = URI(text(v, "target")),
        network: NetworkId = NetworkId.fromBytes(bytes(field("network_id_hex")))) =
        ParticipantEnrollmentHttpCodecV1.context(network, namespace, actor, operation(v), target)
    private fun request(v: JsonObject, body: ByteArray = bytes(field("body_hex")),
        session: ByteArray = bytes(field("session_sha256_hex"))) =
        ParticipantEnrollmentHttpCodecV1.originalRequest(context(v), field("signatory_i105"), field("wallet_i105"),
            field("request_id"), field("idempotency_key"), BigInteger(text(v, "timestamp_ms")), field("nonce"), body, session)
    private fun headers(v: JsonObject): List<ParticipantEnrollmentHttpCodecV1.Header> =
        v.getValue("headers").jsonArray.map { val h = it.jsonObject
            ParticipantEnrollmentHttpCodecV1.Header(text(h, "name"), bytes(text(h, "value_hex"))) }
    private fun decode(v: JsonObject, h: List<ParticipantEnrollmentHttpCodecV1.Header> = headers(v),
        body: ByteArray = bytes(field("body_hex")), method: String = "POST", path: String = text(v, "path"),
        c: ParticipantEnrollmentHttpCodecV1.Context = context(v)) =
        ParticipantEnrollmentHttpCodecV1.decode(c, method, path, body, h)
    private fun replace(v: JsonObject, index: Int, value: ByteArray) = headers(v).mapIndexed { i, h ->
        if (i == index) ParticipantEnrollmentHttpCodecV1.Header(h.name, value) else h }

    @Test fun allThreePurposesMatchExactPublicMessagesAndNineHeaders() {
        assertEquals(3, vectors.size)
        for (v in vectors) {
            val r = request(v)
            assertContentEquals(bytes(text(v, "message_hex")), r.signingMessage())
            assertContentEquals(bytes(field("body_hex")), decode(v).request.originalBody())
            assertContentEquals(bytes(text(v, "signature_hex")), decode(v).originalSignature())
            val encoded = ParticipantEnrollmentHttpCodecV1.encodeHeaders(r, bytes(text(v, "signature_hex")), bytes(field("authorization_hex")))
            assertEquals(9, encoded.size)
            encoded.zip(headers(v)).forEach { (actual, expected) ->
                assertEquals(expected.name, actual.name); assertContentEquals(expected.originalValue(), actual.originalValue())
            }
        }
    }
    @Test fun wrongPurposeGenericPrehashAndRetailSignaturesAreRefused() {
        for (v in vectors) {
            for (other in vectors.filter { it != v })
                assertFails { decode(v, replace(v, 7, text(other, "signature_hex").toByteArray(UTF_8))) }
            for (name in listOf("negative_generic_subject_signature_hex", "negative_iroha_prehash_signature_hex", "negative_retail_raw32_signature_hex"))
                assertFails { decode(v, replace(v, 7, text(v, name).toByteArray(UTF_8))) }
        }
    }
    @Test fun everyActualHeaderMissingOrDuplicatedIsRefusedIncludingMixedCase() {
        val v = vectors.first(); val h = headers(v)
        for (i in h.indices) {
            assertFails { decode(v, h.filterIndexed { index, _ -> index != i }) }
            assertFails { decode(v, h + h[i]) }
            assertFails { decode(v, h + ParticipantEnrollmentHttpCodecV1.Header(h[i].name.toUpperCase(Locale.ROOT), h[i].originalValue())) }
        }
        // HTTP names are case insensitive; a single known spelling remains the same field.
        decode(v, h.map { ParticipantEnrollmentHttpCodecV1.Header(it.name.toUpperCase(Locale.ROOT), it.originalValue()) })
        assertFails { decode(v, h + ParticipantEnrollmentHttpCodecV1.Header("X-Iroha-Enrollment-Unknown", byteArrayOf(49))) }
        decode(v, h + ParticipantEnrollmentHttpCodecV1.Header("X-Dataspace-Id", "is2".toByteArray(UTF_8)))
    }
    @Test fun originalBodyAndEverySignedContextMutationAreRefused() {
        val v = vectors.first()
        for (i in 0..8) {
            val changed = headers(v)[i].originalValue()
            changed[changed.lastIndex] = if (changed.last() == 48.toByte()) 49.toByte() else 48.toByte()
            assertFails { decode(v, replace(v, i, changed)) }
        }
        val body = bytes(field("body_hex")); body[0] = 91
        assertFails { decode(v, body = body) }
        assertFails { decode(v, body = String(bytes(field("body_hex")), UTF_8).trim().toByteArray(UTF_8)) }
        assertFails { decode(v, method = "post") }; assertFails { decode(v, method = "GET") }
        assertFails { decode(v, path = text(v, "path") + "?offered=1") }
        assertFails { decode(v, c = context(v, namespace = "unit-fi-b")) }
        assertFails { decode(v, c = context(v, actor = "other-test-actor")) }
        val otherNetwork = bytes(field("network_id_hex")); otherNetwork[0] = 1
        assertFails { decode(v, c = context(v, network = NetworkId.fromBytes(otherNetwork))) }
        val otherTarget = URI(text(v, "target").replace("unit-fi.invalid", "other-fi.invalid"))
        assertFails { decode(v, c = context(v, target = otherTarget)) }
        val otherMount = URI(text(v, "target").replace("/public-mount/", "/other-mount/"))
        assertFails { decode(v, c = context(v, target = otherMount), path = otherMount.rawPath) }
        assertFails { ParticipantEnrollmentHttpCodecV1.originalRequest(context(v), field("wallet_i105"), field("signatory_i105"),
            field("request_id"), field("idempotency_key"), BigInteger(text(v, "timestamp_ms")), field("nonce"),
            bytes(field("body_hex")), bytes(field("session_sha256_hex"))) }
    }
    @Test fun canonicalUtf8HexDecimalAndVisibleWhitespaceAreEnforced() {
        val v = vectors.first()
        for (i in 0..7) assertFails { decode(v, replace(v, i, byteArrayOf(0xc3.toByte(), 0x28))) }
        for (i in 0..7) for (side in listOf(true, false)) {
            val old = headers(v)[i].originalValue()
            assertFails { decode(v, replace(v, i, if (side) byteArrayOf(32) + old else old + byteArrayOf(32))) }
        }
        for (bad in listOf("0" + text(v, "timestamp_ms"), "18446744073709551616", "30000", "18446744073709521616", "+90001"))
            assertFails { decode(v, replace(v, 5, bad.toByteArray(UTF_8))) }
        for (bad in listOf("AB".repeat(32), "12".repeat(31), "0x" + field("nonce")))
            assertFails { decode(v, replace(v, 6, bad.toByteArray(UTF_8))) }
        for (bad in listOf(text(v, "signature_hex").toUpperCase(Locale.ROOT), "AA==", "00".repeat(64)))
            assertFails { decode(v, replace(v, 7, bad.toByteArray(UTF_8))) }
        assertFails { decode(v, replace(v, 1, field("signatory_canonical_hex").toByteArray(UTF_8))) }
    }
    @Test fun boundsRejectIncompleteOversizedAndNonAsciiOriginals() {
        val v = vectors.first()
        assertFails { request(v, body = byteArrayOf()) }; assertFails { request(v, body = ByteArray(256 * 1024 + 1)) }
        assertFails { request(v, session = ByteArray(32)) }; assertFails { request(v, session = ByteArray(31)) }
        for (auth in listOf(byteArrayOf(), ByteArray(8193) { 65 }, byteArrayOf(31), byteArrayOf(127)))
            assertFails { ParticipantEnrollmentHttpCodecV1.authorizationDigest(auth) }
        assertFails { decode(v, replace(v, 3, ByteArray(8193) { 65 })) }
        assertFails { context(v, namespace = "a".repeat(257)) }; assertFails { context(v, actor = "é") }
        // A public unsigned data fixture can retain non-UTF8 bodies; JSON is never reconstructed.
        val binary = byteArrayOf(0xff.toByte(), 0, 0x80.toByte())
        assertContentEquals(binary, request(v, body = binary).originalBody())
    }
    @Test fun authorizationSpacesAreHashedExactlyAndNeverTrimmed() {
        val v = vectors.first(); val auth = bytes(field("authorization_hex")); val spaced = byteArrayOf(32) + auth + byteArrayOf(32)
        assertContentEquals(java.security.MessageDigest.getInstance("SHA-256").digest(spaced), ParticipantEnrollmentHttpCodecV1.authorizationDigest(spaced))
        assertFails { decode(v, replace(v, 8, spaced)) }
        assertFails { ParticipantEnrollmentHttpCodecV1.encodeHeaders(request(v), bytes(text(v, "signature_hex")), spaced) }
    }
    @Test fun mutableInputsAndReturnedArraysCannotReplaceRetainedOriginals() {
        val v = vectors.first(); val body = bytes(field("body_hex")); val session = bytes(field("session_sha256_hex"))
        val r = request(v, body, session); body.fill(0); session.fill(0)
        r.originalBody().fill(0); r.sessionSha256().fill(0); r.signingMessage().fill(0)
        assertContentEquals(bytes(text(v, "message_hex")), r.signingMessage())
        val signature = bytes(text(v, "signature_hex")); val auth = bytes(field("authorization_hex"))
        val h = ParticipantEnrollmentHttpCodecV1.encodeHeaders(r, signature, auth); signature.fill(0); auth.fill(0)
        h.forEach { it.originalValue().fill(0) }; val received = decode(v, h)
        received.originalSignature().fill(0)
        assertContentEquals(bytes(text(v, "signature_hex")), received.originalSignature())
    }
    @Test fun signedNonUtf8BodyAndAuthorizationSpacesRetainExactOriginals() {
        for (v in fixture.getValue("auxiliary_originals").jsonArray.map { it.jsonObject }) {
            val body = bytes(text(v, "body_hex")); val auth = bytes(text(v, "authorization_hex"))
            val r = request(v, body, bytes(text(v, "session_sha256_hex")))
            assertContentEquals(bytes(text(v, "message_hex")), r.signingMessage())
            val h = ParticipantEnrollmentHttpCodecV1.encodeHeaders(r, bytes(text(v, "signature_hex")), auth)
            assertContentEquals(auth, h.last().originalValue())
            assertContentEquals(body, decode(v, h, body).request.originalBody())
        }
    }
    @Test fun offeredOrNoncanonicalTargetsCannotReplaceExactMountedTarget() {
        val v = vectors.first(); val target = text(v, "target")
        for (bad in listOf(target + "?x=1", target + "#x", target.replace("https://", "http://"),
            target.replace("unit-fi.invalid", "user@unit-fi.invalid"), target.replace("unit-fi.invalid", "unit-fi.invalid:443"),
            target.replace("/public-mount/", "/public-mount/../"), target.replace("/public-mount/", "/public-mount/%2e%2e/")))
            assertFails { context(v, target = URI(bad)) }
    }
}
