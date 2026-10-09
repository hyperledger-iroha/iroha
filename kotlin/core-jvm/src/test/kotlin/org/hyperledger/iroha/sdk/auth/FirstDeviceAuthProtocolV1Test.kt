// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.auth

import java.math.BigInteger
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonObject
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

/** Synthetic protocol DATA tests: no test issues hardware evidence or an authentication session. */
class FirstDeviceAuthProtocolV1Test {
    private val point = hex(
        "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
    )
    private val chain = listOf(byteArrayOf(1, 2, 3), byteArrayOf(4, 5))
    private val der = hex("3006020101020101") // Canonical scalar DATA only; not an issued signature.
    private val token = "synthetic-opaque-token"

    @Test
    fun challengeAndChainMatchActualRustAndPythonGoldens() {
        val challenge = challenge()
        assertEquals(276, challenge.transcriptBytes().size)
        assertEquals(
            "af89ead9caf2a270c0ab37891f2ca0c214f183e5511fc38351ec6ebfff578aab",
            hex(challenge.attestationChallengeBytes()),
        )
        assertEquals(
            "67efd007782515431b15150d20b6899893e492361c824058169c05bd88badbd5",
            hex(FirstDeviceAuthProtocolV1.chainDigestBytes(chain)),
        )
        assertArrayEquals(hex("e803000000000000d007000000000000"), challenge.transcriptBytes().takeLast(16).toByteArray())
    }

    @Test
    fun possessionAndIntegrityMatchActualCanonicalPythonAndIndependentNodeVectors() {
        val challenge = challenge()
        val raw = raw(challenge)
        val message = FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw)
        assertEquals(
            "23c3203119640baf336e2afe81e29595da0b8533f3cad1fc32f285db7e1d2632",
            hex(sha(message)),
        )
        assertEquals(
            "KfxAO_PD-BIsl6q__w1v0pr3L1FlOyenHLueUVD-LQ4",
            FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, raw, message, der),
        )
        assertEquals(
            "29fc403bf3c3f8122c97aabfff0d6fd29af72f51653b27a71cbb9e5150fe2d0e",
            hex(Base64.getUrlDecoder().decode(
                FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, raw, message, der),
            )),
        )
    }

    @Test
    fun eachChallengeDigestAndTimeMutationChangesTheFullAttestationChallenge() {
        val original = challenge()
        val fields = listOf(
            "policy_sha256", "operation_id", "client_nonce", "server_nonce", "alias_digest",
            "google_owner_binding", "google_token_original_sha256",
        )
        for (field in fields) {
            val changed = FirstDeviceAuthProtocolV1.Challenge.parse(
                change(original.originalBytes(), field, Json.of("aa".repeat(32))),
            )
            assertFalse(original.attestationChallengeBytes().contentEquals(changed.attestationChallengeBytes()))
        }
        for (field in listOf("issued_at_ms", "expires_at_ms")) {
            val changed = FirstDeviceAuthProtocolV1.Challenge.parse(
                change(original.originalBytes(), field, Json.of(if (field == "issued_at_ms") 1001 else 2001)),
            )
            assertFalse(original.attestationChallengeBytes().contentEquals(changed.attestationChallengeBytes()))
        }
    }

    @Test
    fun challengeRejectsForeignDuplicateAndNoncanonicalOriginals() {
        val original = challenge().originalBytes().toString(Charsets.UTF_8)
        val variants = listOf(
            original.replaceFirst("{", "{ "),
            original.replaceFirst("{", "{\"version\":1,"),
            original.replaceFirst("{", "{\"foreign\":null,"),
            original.replace("bpng.first", "\\u0062png.first"),
            original + "\n",
            Json.obj((Json.parse(original) as JsonObject).members.entries.reversed()
                .associateTo(LinkedHashMap()) { it.key to it.value }).toJsonString(),
        )
        for (value in variants) rejected { FirstDeviceAuthProtocolV1.Challenge.parse(value.toByteArray()) }
    }

    @Test
    fun challengeRejectsNoncanonicalHexUnknownSchemaAndNonintegerVersions() {
        val original = challenge().originalBytes()
        for (value in listOf("", "00".repeat(32), "AA".repeat(32), "a".repeat(63), "g".repeat(64))) {
            rejected { FirstDeviceAuthProtocolV1.Challenge.parse(change(original, "policy_sha256", Json.of(value))) }
        }
        for (fieldAndValue in listOf(
            "schema" to Json.of("bpng.first-device-auth-challenge.v2"),
            "version" to Json.of(2),
            "version" to Json.parse("1.0"),
            "version" to Json.parse("1e0"),
            "version" to Json.of("1"),
        )) {
            rejected { FirstDeviceAuthProtocolV1.Challenge.parse(change(original, fieldAndValue.first, fieldAndValue.second)) }
        }
    }

    @Test
    fun challengeUnsigned64TimesRetainHighBitsAndNeverWrap() {
        val maximum = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val challenge = challenge(maximum.subtract(BigInteger.valueOf(600000)), maximum)
        assertEquals(maximum, challenge.expiresAtMs)
        assertArrayEquals(ByteArray(8) { -1 }, challenge.transcriptBytes().takeLast(8).toByteArray())
        val original = challenge.originalBytes()
        for (value in listOf("-1", "18446744073709551616", "1e3", "1000.0", "9".repeat(21))) {
            rejected {
                FirstDeviceAuthProtocolV1.Challenge.parse(change(original, "issued_at_ms", Json.parse(value)))
            }
        }
    }

    @Test
    fun challengeLifetimeAndIndependentNonceBoundsAreMandatoryButHistoricalParsingWorks() {
        val original = challenge().originalBytes()
        for ((issued, expires) in listOf(0L to 2000L, 1000L to 1000L, 1000L to 999L, 1000L to 601001L)) {
            rejected {
                FirstDeviceAuthProtocolV1.Challenge.parse(
                    change(change(original, "issued_at_ms", Json.of(issued)), "expires_at_ms", Json.of(expires)),
                )
            }
        }
        rejected {
            FirstDeviceAuthProtocolV1.Challenge.parse(change(original, "server_nonce", Json.of("03".repeat(32))))
        }
        // No local wall-clock check blocks reading an expired retained challenge for recovery.
        assertEquals(BigInteger.valueOf(2000), FirstDeviceAuthProtocolV1.Challenge.parse(original).expiresAtMs)
    }

    @Test
    fun mutableChallengeOriginalAndDerivedArraysCannotAlterRetainedData() {
        val original = challenge().originalBytes()
        val parsed = FirstDeviceAuthProtocolV1.Challenge.parse(original)
        val expected = parsed.transcriptBytes()
        original.fill(0)
        parsed.originalBytes().fill(0)
        parsed.transcriptBytes().fill(0)
        parsed.attestationChallengeBytes().fill(0)
        assertArrayEquals(expected, parsed.transcriptBytes())
    }

    @Test
    fun prepareRequestAndResponseBindExactCallerAliasAndGoogleOriginal() {
        val operation = "02".repeat(32)
        val nonce = "03".repeat(32)
        val alias = "alice@banka.paynet"
        val google = "synthetic-google-original"
        val request = FirstDeviceAuthProtocolV1.prepareRequest(operation, nonce, alias, google)
        assertEquals(operation, request.idempotencyKey)
        assertEquals("/v1/kagemusha/hardware-evidence/first-device/prepare", request.path)
        assertEquals(
            "{\"operation_id\":\"$operation\",\"client_nonce\":\"$nonce\",\"alias\":\"$alias\",\"google_id_token\":\"$google\"}",
            request.bodyBytes().toString(Charsets.UTF_8),
        )
        val expected = challenge(
            overrides = mapOf("alias_digest" to Json.of(hex(sha(alias.toByteArray()))),
                "google_token_original_sha256" to Json.of(hex(sha(google.toByteArray())))),
        )
        val returned = FirstDeviceAuthProtocolV1.parsePrepareResponse(
            response("challenge_original_base64", expected.originalBytes()),
        )
        FirstDeviceAuthProtocolV1.requirePrepareBinding(returned, operation, nonce, alias, google)
        for ((op, n, a, g) in listOf(
            listOf("0a".repeat(32), nonce, alias, google),
            listOf(operation, "0b".repeat(32), alias, google),
            listOf(operation, nonce, alias + "x", google),
            listOf(operation, nonce, alias, google + "x"),
        )) rejected { FirstDeviceAuthProtocolV1.requirePrepareBinding(returned, op, n, a, g) }
    }

    @Test
    fun prepareRejectsUnboundedNonGraphicOrNoncanonicalCallerData() {
        for ((op, nonce, alias, google) in listOf(
            listOf("00".repeat(32), "03".repeat(32), "a", "token"),
            listOf("AA".repeat(32), "03".repeat(32), "a", "token"),
            listOf("02".repeat(32), "03".repeat(32), "a b", "token"),
            listOf("02".repeat(32), "03".repeat(32), "a".repeat(257), "token"),
            listOf("02".repeat(32), "03".repeat(32), "a", "token\n"),
            listOf("02".repeat(32), "03".repeat(32), "a", "t".repeat(16385)),
        )) rejected { FirstDeviceAuthProtocolV1.prepareRequest(op, nonce, alias, google) }
    }

    @Test
    fun rawResponseRetainsOriginalOrderingWhitespaceAndEveryArrayIndependently() {
        val source = rawBytes(challenge())
        val withWhitespace = (" " + source.toString(Charsets.UTF_8) + "\n").toByteArray()
        val parsed = FirstDeviceAuthProtocolV1.RawOriginal.parse(withWhitespace)
        val expected = withWhitespace.copyOf()
        withWhitespace.fill(0)
        parsed.originalBytes().fill(0)
        parsed.appPublicKeySec1Bytes().fill(0)
        parsed.certificateChainDerBytes().forEach { it.fill(0) }
        assertArrayEquals(expected, parsed.originalBytes())
        assertArrayEquals(point, parsed.appPublicKeySec1Bytes())
        FirstDeviceAuthProtocolV1.requireRawChainBinding(parsed, chain)
    }

    @Test
    fun rawClosedFieldsCanonicalBase64AndActualP256PointAreRequired() {
        val original = rawBytes(challenge())
        val replacements = listOf(
            "app_public_key_sec1_base64" to Json.of(b64(point).trimEnd('=')),
            "app_public_key_sec1_base64" to Json.of(b64(point) + "\n"),
            "app_public_key_sec1_base64" to Json.of(b64(ByteArray(65) { if (it == 0) 4 else 0 })),
            "app_public_key_sec1_base64" to Json.of(b64(point.copyOfRange(1, 65))),
            "security_level" to Json.of(0),
            "security_level" to Json.of(3),
            "security_level" to Json.of(true),
            "checked_at_ms" to Json.of(-1),
            "checked_at_ms" to Json.parse("18446744073709551616"),
            "original_chain_base64" to Json.array(Json.of("AQID")),
            "original_chain_base64" to Json.array(Json.of(""), Json.of("BAU=")),
            "original_chain_base64" to Json.array(Json.of("AR=="), Json.of("BAU=")),
            "original_chain_base64" to Json.array(Json.of("AQID"), Json.NULL),
        )
        for ((field, value) in replacements) {
            rejected { FirstDeviceAuthProtocolV1.RawOriginal.parse(change(original, field, value)) }
        }
        rejected { FirstDeviceAuthProtocolV1.RawOriginal.parse(add(original, "ready", Json.of(true))) }
        rejected { FirstDeviceAuthProtocolV1.RawOriginal.parse(original.toString(Charsets.UTF_8)
            .replaceFirst("{", "{\"version\":1,").toByteArray()) }
    }

    @Test
    fun rawChallengeTimeAndSubmittedChainBindingCannotBeRetargeted() {
        val challenge = challenge()
        val original = rawBytes(challenge)
        for ((field, value) in listOf(
            "challenge_digest" to Json.of("aa".repeat(32)),
            "checked_at_ms" to Json.of(999),
            "checked_at_ms" to Json.of(2000),
        )) {
            val altered = FirstDeviceAuthProtocolV1.RawOriginal.parse(change(original, field, value))
            rejected { FirstDeviceAuthProtocolV1.requireRawBinding(challenge, altered) }
            rejected { FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, altered) }
        }
        val raw = FirstDeviceAuthProtocolV1.RawOriginal.parse(original)
        rejected { FirstDeviceAuthProtocolV1.requireRawChainBinding(raw, chain.reversed()) }
        rejected { FirstDeviceAuthProtocolV1.requireRawChainBinding(raw, listOf(byteArrayOf(1, 2), byteArrayOf(3, 4, 5))) }
    }

    @Test
    fun chainDigestBindsDomainCountOrderAndLengthInsteadOfConcatenation() {
        val digest = FirstDeviceAuthProtocolV1.chainDigestBytes(chain)
        assertFalse(digest.contentEquals(FirstDeviceAuthProtocolV1.chainDigestBytes(chain.reversed())))
        assertFalse(digest.contentEquals(FirstDeviceAuthProtocolV1.chainDigestBytes(
            listOf(byteArrayOf(1, 2), byteArrayOf(3, 4, 5)),
        )))
        for (bad in listOf(
            emptyList(), listOf(byteArrayOf(1)), List(9) { byteArrayOf(1) },
            listOf(byteArrayOf(), byteArrayOf(1)), listOf(ByteArray(16385), byteArrayOf(1)),
        )) rejected { FirstDeviceAuthProtocolV1.chainDigestBytes(bad) }
        assertEquals(32, FirstDeviceAuthProtocolV1.chainDigestBytes(List(8) { ByteArray(16384) }).size)
    }

    @Test
    fun canonicalDerPreservesHighSAndRejectsNormalizationOrNonminimalEncoding() {
        val highS = hex(
            "3026020101022100ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632550",
        )
        val copied = FirstDeviceAuthProtocolV1.canonicalPossessionDerBytes(highS)
        assertArrayEquals(highS, copied)
        copied.fill(0)
        assertArrayEquals(der, FirstDeviceAuthProtocolV1.canonicalPossessionDerBytes(der))
        for (bad in listOf(
            hex("300702020001020101"), hex("3006020100020101"), hex("3006020180020101"),
            hex("308106020101020101"), der + byteArrayOf(0), ByteArray(73),
            hex("3026020101022100ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551"),
        )) rejected { FirstDeviceAuthProtocolV1.canonicalPossessionDerBytes(bad) }
    }

    @Test
    fun integrityHashBindsExactRawOriginalAndOriginalDerWithoutReencoding() {
        val challenge = challenge()
        val raw = raw(challenge)
        val message = FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw)
        val originalHash = FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, raw, message, der)
        val reorderedRaw = FirstDeviceAuthProtocolV1.RawOriginal.parse(
            (" " + raw.originalBytes().toString(Charsets.UTF_8)).toByteArray(),
        )
        assertNotEquals(originalHash, FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, reorderedRaw, message, der))
        assertNotEquals(originalHash, FirstDeviceAuthProtocolV1.integrityRequestHashText(
            challenge, raw, message, hex("3006020101020102"),
        ))
        val modified = message.copyOf().also { it[it.lastIndex] = 0 }
        rejected { FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, raw, modified, der) }
        assertEquals(43, originalHash.length)
        assertFalse(originalHash.contains('='))
    }

    @Test
    fun rawAndFinishHttpBodiesPreserveOriginalsAndExactCurrentFieldNames() {
        val challenge = challenge()
        val submitted = chain.map { it.copyOf() }.toMutableList()
        val request = FirstDeviceAuthProtocolV1.rawAttestationRequest(challenge, submitted)
        submitted.forEach { it.fill(0) }
        submitted.clear()
        val body = Json.parse(request.bodyBytes()) as JsonObject
        assertEquals(setOf("challenge_original_base64", "certificate_chain_der_base64"), body.keys)
        assertEquals("AQID", ((body["certificate_chain_der_base64"] as org.hyperledger.iroha.sdk.json.JsonArray).items[0]
            as org.hyperledger.iroha.sdk.json.JsonString).value)
        val raw = raw(challenge)
        val message = FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw)
        val signature = der.copyOf()
        val finish = FirstDeviceAuthProtocolV1.finishRequest(challenge, raw, message, signature, token)
        val retained = finish.bodyBytes()
        message.fill(0)
        signature.fill(0)
        finish.bodyBytes().fill(0)
        assertArrayEquals(retained, finish.bodyBytes())
        assertEquals(challenge.operationId, finish.idempotencyKey)
        assertEquals(
            setOf("challenge_original_base64", "raw_verifier_original_base64",
                "possession_message_original_base64", "possession_signature_der_base64", "play_integrity_token"),
            (Json.parse(retained) as JsonObject).keys,
        )
    }

    @Test
    fun finishHistoricalOriginalAndGoogleResponseCannotBeMutatedOrExceedActualCap() {
        val challenge = challenge()
        val raw = raw(challenge)
        val bytes = finishBytes(challenge, raw)
        val parsed = FirstDeviceAuthProtocolV1.FinishOriginal.parse(bytes)
        val retained = bytes.copyOf()
        bytes.fill(0)
        parsed.originalBytes().fill(0)
        parsed.googleResponseOriginalBytes().fill(0)
        assertArrayEquals(retained, parsed.originalBytes())
        assertArrayEquals("synthetic Google original".toByteArray(), parsed.googleResponseOriginalBytes())
        val capped = change(retained, "google_response_original_base64", Json.of(b64(ByteArray(128 * 1024))))
        assertEquals(128 * 1024, FirstDeviceAuthProtocolV1.FinishOriginal.parse(capped).googleResponseOriginalBytes().size)
        rejected {
            FirstDeviceAuthProtocolV1.FinishOriginal.parse(change(
                retained, "google_response_original_base64", Json.of(b64(ByteArray(128 * 1024 + 1))),
            ))
        }
    }

    @Test
    fun finishBindingRejectsEveryReplacedDigestConfigAndCaptureTime() {
        val challenge = challenge()
        val raw = raw(challenge)
        val message = FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw)
        val original = finishBytes(challenge, raw)
        val parsed = FirstDeviceAuthProtocolV1.FinishOriginal.parse(original)
        FirstDeviceAuthProtocolV1.requireFinishBinding(challenge, raw, message, der, token, parsed)
        for (field in listOf(
            "config_sha256", "challenge_digest", "raw_verifier_original_sha256",
            "possession_message_sha256", "possession_der_sha256", "token_original_sha256", "integrity_request_hash",
        )) {
            val changed = FirstDeviceAuthProtocolV1.FinishOriginal.parse(change(original, field, Json.of("aa".repeat(32))))
            rejected { FirstDeviceAuthProtocolV1.requireFinishBinding(challenge, raw, message, der, token, changed) }
        }
        for (time in listOf(1199, 2000)) {
            val changed = FirstDeviceAuthProtocolV1.FinishOriginal.parse(change(original, "verified_at_ms", Json.of(time)))
            rejected { FirstDeviceAuthProtocolV1.requireFinishBinding(challenge, raw, message, der, token, changed) }
        }
        rejected { FirstDeviceAuthProtocolV1.requireFinishBinding(challenge, raw, message, der, token + "x", parsed) }
    }

    @Test
    fun finishParserRejectsForeignMissingWrongTypeAndNoncanonicalDigestFields() {
        val original = finishBytes(challenge(), raw(challenge()))
        for ((field, value) in listOf(
            "schema" to Json.of("bpng.first-device-auth-raw.v1"),
            "version" to Json.of(2),
            "config_sha256" to Json.of("00".repeat(32)),
            "integrity_request_hash" to Json.of("AA".repeat(32)),
            "verified_at_ms" to Json.of("1500"),
            "google_response_original_base64" to Json.of("YQ"),
            "google_response_original_base64" to Json.NULL,
        )) rejected { FirstDeviceAuthProtocolV1.FinishOriginal.parse(change(original, field, value)) }
        rejected { FirstDeviceAuthProtocolV1.FinishOriginal.parse(add(original, "verified", Json.of(true))) }
        val missing = (Json.parse(original) as JsonObject).members.toMutableMap().also { it.remove("version") }
        rejected { FirstDeviceAuthProtocolV1.FinishOriginal.parse(Json.obj(missing).toJsonBytes()) }
    }

    @Test
    fun responseExplicitNullIsRequiredAndCannotBeTreatedAsAnOriginal() {
        val nullResult = "{\"result_original_base64\":null}".toByteArray()
        assertNull(FirstDeviceAuthProtocolV1.parseRawAttestationResponse(nullResult))
        assertNull(FirstDeviceAuthProtocolV1.parseFinishResponse(nullResult))
        assertNull(FirstDeviceAuthProtocolV1.parseRecoveryResponse("{\"recovered_original_base64\":null}".toByteArray()))
        for (body in listOf(
            "{}", "{\"result_original_base64\":true}", "{\"result_original_base64\":\"\"}",
            "{\"result_original_base64\":null,\"ready\":true}",
            "{\"result_original_base64\":null,\"result_original_base64\":null}",
            "{\"result_original_base64\":\"YR==\"}",
        )) rejected { FirstDeviceAuthProtocolV1.parseRawAttestationResponse(body.toByteArray()) }
        val challenge = challenge()
        assertEquals(raw(challenge), FirstDeviceAuthProtocolV1.parseRawAttestationResponse(
            response("result_original_base64", raw(challenge).originalBytes()),
        ))
        assertEquals(
            FirstDeviceAuthProtocolV1.FinishOriginal.parse(finishBytes(challenge, raw(challenge))),
            FirstDeviceAuthProtocolV1.parseFinishResponse(response("result_original_base64", finishBytes(challenge, raw(challenge)))),
        )
    }

    @Test
    fun recoveryTransmitsExactRetainedChallengeAndDoesNotRenewHistoricalTimes() {
        val challenge = challenge()
        for (phase in FirstDeviceAuthProtocolV1.RecoveryPhase.values()) {
            val request = FirstDeviceAuthProtocolV1.recoveryRequest(challenge, "fresh-synthetic-google-token", phase)
            val obj = Json.parse(request.bodyBytes()) as JsonObject
            assertEquals(setOf("challenge_original_base64", "google_id_token", "phase"), obj.keys)
            assertEquals(phase.wireName, obj.stringOrNull("phase"))
            assertArrayEquals(challenge.originalBytes(), Base64.getDecoder().decode(obj.stringOrNull("challenge_original_base64")))
            assertEquals(challenge.operationId, request.idempotencyKey)
        }
        val returned = FirstDeviceAuthProtocolV1.parseRecoveryResponse(
            response("recovered_original_base64", challenge.originalBytes()),
        )!!
        assertArrayEquals(challenge.originalBytes(), returned)
        returned.fill(0)
        assertEquals(BigInteger.valueOf(1000), challenge.issuedAtMs)
    }

    @Test
    fun lostPrepareRecoveryCarriesInitialBindingsWithoutAChallenge() {
        val operation = "02".repeat(32)
        val nonce = "03".repeat(32)
        val initial = FirstDeviceAuthProtocolV1.googleIdTokenOriginalSha256("initial-google-original")
        val request = FirstDeviceAuthProtocolV1.prepareRecoveryRequest(operation, nonce,
            "original-auth-alias", initial, "fresh-google-original")
        val obj = Json.parse(request.bodyBytes()) as JsonObject
        assertEquals(setOf("phase", "operation_id", "client_nonce", "alias",
            "google_token_original_sha256", "google_id_token"), obj.keys)
        assertEquals("prepare", obj.stringOrNull("phase"))
        assertEquals(initial, obj.stringOrNull("google_token_original_sha256"))
        assertEquals("fresh-google-original", obj.stringOrNull("google_id_token"))
        assertFalse(obj.containsKey("challenge_original_base64"))
        assertEquals(operation, request.idempotencyKey)
        assertEquals("/v1/kagemusha/hardware-evidence/first-device/recover", request.path)
        assertNotEquals(initial, FirstDeviceAuthProtocolV1.googleIdTokenOriginalSha256("fresh-google-original"))
        val copy = request.bodyBytes(); copy.fill(0)
        assertEquals(obj, Json.parse(request.bodyBytes()))
    }

    @Test
    fun recoveredPrepareMatchesInitialDataAndKeepsHistoricalTimes() {
        val alias = "original-auth-alias"
        val initial = FirstDeviceAuthProtocolV1.googleIdTokenOriginalSha256("initial-google-original")
        val value = challenge(overrides = mapOf(
            "alias_digest" to Json.of(hex(sha(alias.toByteArray(Charsets.US_ASCII)))),
            "google_token_original_sha256" to Json.of(initial),
        ))
        FirstDeviceAuthProtocolV1.requirePrepareRecoveryBinding(value, value.operationId,
            value.clientNonce, alias, initial)
        assertEquals(BigInteger.valueOf(1000), value.issuedAtMs)
        assertEquals(BigInteger.valueOf(2000), value.expiresAtMs)
        for ((operation, nonce, offeredAlias, tokenDigest) in listOf(
            listOf("09".repeat(32), value.clientNonce, alias, initial),
            listOf(value.operationId, "09".repeat(32), alias, initial),
            listOf(value.operationId, value.clientNonce, "other-alias", initial),
            listOf(value.operationId, value.clientNonce, alias,
                FirstDeviceAuthProtocolV1.googleIdTokenOriginalSha256("fresh-google-original")),
        )) rejected { FirstDeviceAuthProtocolV1.requirePrepareRecoveryBinding(value,
            operation, nonce, offeredAlias, tokenDigest) }
    }

    @Test
    fun prepareRecoveryRejectsInvalidOriginalBindingsAndFreshTokens() {
        fun request(operation: String = "02".repeat(32), nonce: String = "03".repeat(32),
            alias: String = "original-alias", digest: String = "04".repeat(32), fresh: String = "fresh") =
            FirstDeviceAuthProtocolV1.prepareRecoveryRequest(operation, nonce, alias, digest, fresh)
        for (bad in listOf("00".repeat(32), "AB".repeat(32), "0a".repeat(31), "gg".repeat(32))) {
            rejected { request(operation = bad) }; rejected { request(nonce = bad) }; rejected { request(digest = bad) }
        }
        for (bad in listOf("", " ", "a".repeat(257), "alias-α")) rejected { request(alias = bad) }
        for (bad in listOf("", "token\n", "t".repeat(16385))) {
            rejected { request(fresh = bad) }; rejected { FirstDeviceAuthProtocolV1.googleIdTokenOriginalSha256(bad) }
        }
        assertEquals(listOf("raw-attestation", "finish"),
            FirstDeviceAuthProtocolV1.RecoveryPhase.values().map { it.wireName })
    }

    @Test
    fun malformedUtf8AndBoundViolationsAreRejectedBeforeDataExposure() {
        rejected { FirstDeviceAuthProtocolV1.Challenge.parse(byteArrayOf(0xff.toByte())) }
        rejected { FirstDeviceAuthProtocolV1.Challenge.parse(ByteArray(4097)) }
        rejected { FirstDeviceAuthProtocolV1.RawOriginal.parse(ByteArray(360 * 1024 + 1)) }
        rejected { FirstDeviceAuthProtocolV1.parseRecoveryResponse(ByteArray(1024 * 1024 + 1)) }
        rejected { FirstDeviceAuthProtocolV1.parsePrepareResponse("{\"challenge_original_base64\":null}".toByteArray()) }
        rejected { FirstDeviceAuthProtocolV1.recoveryRequest(challenge(), "token\n", FirstDeviceAuthProtocolV1.RecoveryPhase.FINISH) }
        val challenge = challenge()
        val raw = raw(challenge)
        rejected { FirstDeviceAuthProtocolV1.finishRequest(challenge, raw,
            FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw), der, "t".repeat(65537)) }
    }

    private fun challenge(
        issued: BigInteger = BigInteger.valueOf(1000),
        expires: BigInteger = BigInteger.valueOf(2000),
        overrides: Map<String, Json> = emptyMap(),
    ): FirstDeviceAuthProtocolV1.Challenge {
        val fields = linkedMapOf<String, Json>(
            "schema" to Json.of("bpng.first-device-auth-challenge.v1"), "version" to Json.of(1),
        )
        listOf("policy_sha256", "operation_id", "client_nonce", "server_nonce", "alias_digest",
            "google_owner_binding", "google_token_original_sha256").forEachIndexed { index, field ->
            fields[field] = Json.of("%02x".format(index + 1).repeat(32))
        }
        fields["issued_at_ms"] = Json.of(issued)
        fields["expires_at_ms"] = Json.of(expires)
        fields.putAll(overrides)
        return FirstDeviceAuthProtocolV1.Challenge.parse(Json.obj(fields).toJsonBytes())
    }

    private fun raw(challenge: FirstDeviceAuthProtocolV1.Challenge): FirstDeviceAuthProtocolV1.RawOriginal =
        FirstDeviceAuthProtocolV1.RawOriginal.parse(rawBytes(challenge))

    private fun rawBytes(challenge: FirstDeviceAuthProtocolV1.Challenge): ByteArray = Json.obj(
        sortedMapOf(
            "schema" to Json.of("bpng.first-device-auth-raw.v1"), "version" to Json.of(1),
            "config_sha256" to Json.of("08".repeat(32)),
            "challenge_digest" to Json.of(hex(challenge.attestationChallengeBytes())),
            "raw_request_sha256" to Json.of("09".repeat(32)),
            "app_public_key_sec1_base64" to Json.of(b64(point)), "security_level" to Json.of(2),
            "checked_at_ms" to Json.of(1200),
            "original_chain_base64" to Json.array(chain.map { Json.of(b64(it)) }),
        ),
    ).toJsonBytes()

    private fun finishBytes(
        challenge: FirstDeviceAuthProtocolV1.Challenge,
        raw: FirstDeviceAuthProtocolV1.RawOriginal,
    ): ByteArray {
        val message = FirstDeviceAuthProtocolV1.possessionMessageBytes(challenge, raw)
        val hash = Base64.getUrlDecoder().decode(FirstDeviceAuthProtocolV1.integrityRequestHashText(challenge, raw, message, der))
        return Json.obj(sortedMapOf(
            "schema" to Json.of("bpng.first-device-auth-finish.v1"), "version" to Json.of(1),
            "config_sha256" to Json.of(raw.configSha256),
            "challenge_digest" to Json.of(hex(challenge.attestationChallengeBytes())),
            "raw_verifier_original_sha256" to Json.of(hex(sha(raw.originalBytes()))),
            "possession_message_sha256" to Json.of(hex(sha(message))),
            "possession_der_sha256" to Json.of(hex(sha(der))),
            "token_original_sha256" to Json.of(hex(sha(token.toByteArray()))),
            "integrity_request_hash" to Json.of(hex(hash)), "verified_at_ms" to Json.of(1500),
            "google_response_original_base64" to Json.of(b64("synthetic Google original".toByteArray())),
        )).toJsonBytes()
    }

    private fun change(original: ByteArray, field: String, value: Json): ByteArray {
        val members = (Json.parse(original) as JsonObject).members.toMutableMap()
        members[field] = value
        return Json.obj(members).toJsonBytes()
    }

    private fun add(original: ByteArray, field: String, value: Json): ByteArray =
        change(original, field, value)

    private fun response(field: String, original: ByteArray): ByteArray =
        Json.obj(mapOf(field to Json.of(b64(original)))).toJsonBytes()

    private fun rejected(action: () -> Unit) {
        assertThrows(IllegalArgumentException::class.java, action)
    }

    private fun b64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)

    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun hex(text: String): ByteArray =
        ByteArray(text.length / 2) { text.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}
