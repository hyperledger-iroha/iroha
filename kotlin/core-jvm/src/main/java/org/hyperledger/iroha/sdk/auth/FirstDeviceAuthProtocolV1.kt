// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.auth

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec

/**
 * First-device authentication V1 data boundary, independent of Wallet E1 and payment custody.
 *
 * Parsing supplies historical data only. It does not verify Google authorization, attestation,
 * Integrity, enrolled ownership or a released runtime. Only Core's private retained verifier and
 * protected DATA establish those facts. An authentication-purpose P-256 key is a separate role;
 * none of these values creates a Wallet generation grant or replaces the existing Ed25519 account.
 *
 * This class contains no HTTP transport, key creation, signing, persistence or success constructor.
 * Platform and application callers retain exact originals for retry and recovery.
 */
object FirstDeviceAuthProtocolV1 {
    const val CHALLENGE_TRANSCRIPT_BYTES: Int = 276
    const val MAX_ORIGINAL_BYTES: Int = 360 * 1024
    const val MAX_HTTP_BODY_BYTES: Int = 1024 * 1024
    const val MAX_CERTIFICATE_BYTES: Int = 16384
    const val MAX_GOOGLE_RESPONSE_BYTES: Int = 128 * 1024

    private const val ROUTE = "/v1/kagemusha/hardware-evidence/first-device"
    private const val CHALLENGE_SCHEMA = "bpng.first-device-auth-challenge.v1"
    private const val RAW_SCHEMA = "bpng.first-device-auth-raw.v1"
    private const val FINISH_SCHEMA = "bpng.first-device-auth-finish.v1"
    private val CHALLENGE_DOMAIN = ascii("BPNG.FIRST_DEVICE.AUTH.CHALLENGE.V1\u0000")
    private val CHAIN_DOMAIN = ascii("BPNG.FIRST_DEVICE.AUTH.CHAIN.V1\u0000")
    private val POSSESSION_DOMAIN = ascii("BPNG.FIRST_DEVICE.AUTH.POSSESSION.V1\u0000")
    private val INTEGRITY_DOMAIN = ascii("BPNG.FIRST_DEVICE.AUTH.INTEGRITY.V1\u0000")
    private val MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val MAX_LIFETIME = BigInteger.valueOf(600000)

    private val CHALLENGE_FIELDS = listOf(
        "schema", "version", "policy_sha256", "operation_id", "client_nonce", "server_nonce",
        "alias_digest", "google_owner_binding", "google_token_original_sha256",
        "issued_at_ms", "expires_at_ms",
    )
    private val RAW_FIELDS = setOf(
        "schema", "version", "config_sha256", "challenge_digest", "raw_request_sha256",
        "app_public_key_sec1_base64", "security_level", "checked_at_ms", "original_chain_base64",
    )
    private val FINISH_FIELDS = setOf(
        "schema", "version", "config_sha256", "challenge_digest", "raw_verifier_original_sha256",
        "possession_message_sha256", "possession_der_sha256", "token_original_sha256",
        "integrity_request_hash", "verified_at_ms", "google_response_original_base64",
    )

    /** Exact retained Core challenge data. Construction is available only through parsing. */
    class Challenge private constructor(
        original: ByteArray,
        val policySha256: String,
        val operationId: String,
        val clientNonce: String,
        val serverNonce: String,
        val aliasDigest: String,
        val googleOwnerBinding: String,
        val googleTokenOriginalSha256: String,
        val issuedAtMs: BigInteger,
        val expiresAtMs: BigInteger,
    ) {
        private val retainedOriginal = original.copyOf()

        /** Exact canonical JSON original from Core, copied on access. */
        fun originalBytes(): ByteArray = retainedOriginal.copyOf()

        /** Exact domain, seven digests and unsigned little-endian timestamps. */
        fun transcriptBytes(): ByteArray = ByteArrayOutputStream(CHALLENGE_TRANSCRIPT_BYTES).apply {
            write(CHALLENGE_DOMAIN)
            for (field in listOf(
                policySha256, operationId, clientNonce, serverNonce, aliasDigest,
                googleOwnerBinding, googleTokenOriginalSha256,
            )) write(digest(field))
            write(u64le(issuedAtMs))
            write(u64le(expiresAtMs))
        }.toByteArray().also { check(it.size == CHALLENGE_TRANSCRIPT_BYTES) }

        /** Full-transcript SHA256 supplied to the separate authentication-key attestation. */
        fun attestationChallengeBytes(): ByteArray = sha(transcriptBytes())

        override fun equals(other: Any?): Boolean =
            other is Challenge && retainedOriginal.contentEquals(other.retainedOriginal)

        override fun hashCode(): Int = retainedOriginal.contentHashCode()

        companion object {
            /**
             * Parse the exact compact ordered Core original, with closed fields and finite u64 time.
             * This does not check current time or establish the challenge's authenticity.
             */
            @JvmStatic
            fun parse(original: ByteArray): Challenge {
                val bytes = boundedCopy(original, 4096)
                val obj = objectFrom(bytes, CHALLENGE_FIELDS.toSet())
                schema(obj, CHALLENGE_SCHEMA)
                val digests = CHALLENGE_FIELDS.subList(2, 9).map { digestText(obj, it) }
                val issued = u64(obj, "issued_at_ms")
                val expires = u64(obj, "expires_at_ms")
                require(issued.signum() > 0 && expires > issued && expires.subtract(issued) <= MAX_LIFETIME) {
                    "first-device auth challenge lifetime is not finite"
                }
                require(digests[2] != digests[3]) { "first-device auth nonces must be independent" }
                val ordered = LinkedHashMap<String, Json>()
                for (field in CHALLENGE_FIELDS) ordered[field] = obj[field]!!
                require(Json.obj(ordered).toJsonBytes().contentEquals(bytes)) {
                    "first-device auth challenge original is not canonical Core JSON"
                }
                return Challenge(
                    bytes, digests[0], digests[1], digests[2], digests[3], digests[4], digests[5],
                    digests[6], issued, expires,
                )
            }
        }
    }

    /** Historical raw-verifier response data, retaining the entire returned original unchanged. */
    class RawOriginal private constructor(
        original: ByteArray,
        val configSha256: String,
        val challengeDigest: String,
        val rawRequestSha256: String,
        point: ByteArray,
        val securityLevel: Int,
        val checkedAtMs: BigInteger,
        chain: List<ByteArray>,
    ) {
        private val retainedOriginal = original.copyOf()
        private val retainedPoint = point.copyOf()
        private val retainedChain = copyChain(chain)

        fun originalBytes(): ByteArray = retainedOriginal.copyOf()

        /** Authentication-role public point only; it conveys no hardware or wallet verdict. */
        fun appPublicKeySec1Bytes(): ByteArray = retainedPoint.copyOf()

        fun certificateChainDerBytes(): List<ByteArray> = copyChain(retainedChain)

        override fun equals(other: Any?): Boolean =
            other is RawOriginal && retainedOriginal.contentEquals(other.retainedOriginal)

        override fun hashCode(): Int = retainedOriginal.contentHashCode()

        companion object {
            /** Closed, bounded parsing only. No chain, revocation, hardware or app verification. */
            @JvmStatic
            fun parse(original: ByteArray): RawOriginal {
                val bytes = boundedCopy(original, MAX_ORIGINAL_BYTES)
                val obj = objectFrom(bytes, RAW_FIELDS)
                schema(obj, RAW_SCHEMA)
                val point = KagemushaP256Codec.requireUncompressedPublicKey(
                    binary(string(obj, "app_public_key_sec1_base64"), 65, 65),
                )
                val level = smallInteger(obj, "security_level")
                require(level == 1 || level == 2) { "first-device auth security-level encoding is invalid" }
                return RawOriginal(
                    bytes, digestText(obj, "config_sha256"), digestText(obj, "challenge_digest"),
                    digestText(obj, "raw_request_sha256"), point, level, u64(obj, "checked_at_ms"),
                    chainFrom(obj, "original_chain_base64"),
                )
            }
        }
    }

    /** Historical finish-verifier response data; no public opaque verified-result constructor. */
    class FinishOriginal private constructor(
        original: ByteArray,
        val configSha256: String,
        val challengeDigest: String,
        val rawVerifierOriginalSha256: String,
        val possessionMessageSha256: String,
        val possessionDerSha256: String,
        val tokenOriginalSha256: String,
        val integrityRequestHash: String,
        val verifiedAtMs: BigInteger,
        googleResponse: ByteArray,
    ) {
        private val retainedOriginal = original.copyOf()
        private val retainedGoogleResponse = googleResponse.copyOf()

        fun originalBytes(): ByteArray = retainedOriginal.copyOf()

        /** Google response original data, without a decoded verdict or application authority. */
        fun googleResponseOriginalBytes(): ByteArray = retainedGoogleResponse.copyOf()

        override fun equals(other: Any?): Boolean =
            other is FinishOriginal && retainedOriginal.contentEquals(other.retainedOriginal)

        override fun hashCode(): Int = retainedOriginal.contentHashCode()

        companion object {
            /** Parse the exact returned original without rebuilding or interpreting Google results. */
            @JvmStatic
            fun parse(original: ByteArray): FinishOriginal {
                val bytes = boundedCopy(original, MAX_ORIGINAL_BYTES)
                val obj = objectFrom(bytes, FINISH_FIELDS)
                schema(obj, FINISH_SCHEMA)
                return FinishOriginal(
                    bytes, digestText(obj, "config_sha256"), digestText(obj, "challenge_digest"),
                    digestText(obj, "raw_verifier_original_sha256"),
                    digestText(obj, "possession_message_sha256"), digestText(obj, "possession_der_sha256"),
                    digestText(obj, "token_original_sha256"), digestText(obj, "integrity_request_hash"),
                    u64(obj, "verified_at_ms"),
                    binary(string(obj, "google_response_original_base64"), MAX_GOOGLE_RESPONSE_BYTES),
                )
            }
        }
    }

    /** Immutable POST body and exact idempotency correlation data; no transport or authorization. */
    class HttpRequestData internal constructor(
        val path: String,
        val idempotencyKey: String,
        body: ByteArray,
    ) {
        private val retainedBody = boundedCopy(body, MAX_HTTP_BODY_BYTES)

        fun bodyBytes(): ByteArray = retainedBody.copyOf()

        override fun equals(other: Any?): Boolean =
            other is HttpRequestData && path == other.path && idempotencyKey == other.idempotencyKey &&
                retainedBody.contentEquals(other.retainedBody)

        override fun hashCode(): Int =
            31 * (31 * path.hashCode() + idempotencyKey.hashCode()) + retainedBody.contentHashCode()
    }

    /** Recovery selects exact retained phase data; it never renews or re-verifies a consumed attempt. */
    enum class RecoveryPhase(val wireName: String) {
        PREPARE("prepare"),
        RAW_ATTESTATION("raw-attestation"),
        FINISH("finish"),
    }

    /** Pure prepare request data with independent nonzero caller-generated random identifiers. */
    @JvmStatic
    fun prepareRequest(
        operationId: String,
        clientNonce: String,
        alias: String,
        googleIdToken: String,
    ): HttpRequestData {
        digest(operationId)
        digest(clientNonce)
        graphic(alias, 256)
        graphic(googleIdToken, 16384)
        return request(
            "/prepare", operationId, linkedMapOf(
                "operation_id" to Json.of(operationId), "client_nonce" to Json.of(clientNonce),
                "alias" to Json.of(alias), "google_id_token" to Json.of(googleIdToken),
            ),
        )
    }

    /** The prepare response must contain its sole original field; parsing grants no authority. */
    @JvmStatic
    fun parsePrepareResponse(body: ByteArray): Challenge {
        val obj = objectFrom(boundedCopy(body, MAX_HTTP_BODY_BYTES), setOf("challenge_original_base64"))
        return Challenge.parse(binary(string(obj, "challenge_original_base64"), 4096))
    }

    /** Pure raw-attestation HTTP body, preserving the original leaf-first DER chain. */
    @JvmStatic
    fun rawAttestationRequest(challenge: Challenge, chain: List<ByteArray>): HttpRequestData {
        val copiedChain = copyChain(chain)
        chainDigestBytes(copiedChain)
        return request(
            "/raw-attestation", challenge.operationId, linkedMapOf(
                "challenge_original_base64" to Json.of(standard(challenge.originalBytes())),
                "certificate_chain_der_base64" to encodedChain(copiedChain),
            ),
        )
    }

    /** Mandatory explicit null remains unavailable data; absent or foreign fields are rejected. */
    @JvmStatic
    fun parseRawAttestationResponse(body: ByteArray): RawOriginal? =
        nullableOriginal(body, "result_original_base64")?.let { RawOriginal.parse(it) }

    /** Count, order and each original DER length are bound by the CHAIN domain and SHA256. */
    @JvmStatic
    fun chainDigestBytes(chain: List<ByteArray>): ByteArray {
        val originals = copyChain(chain)
        require(originals.size in 2..8) { "first-device auth certificate count is out of bounds" }
        val framed = ByteArrayOutputStream().apply {
            write(CHAIN_DOMAIN)
            write(u32le(originals.size))
            for (item in originals) {
                require(item.isNotEmpty() && item.size <= MAX_CERTIFICATE_BYTES) {
                    "first-device auth certificate original is out of bounds"
                }
                write(u32le(item.size))
                write(item)
            }
        }.toByteArray()
        return sha(framed)
    }

    /**
     * Full authentication-role possession message, derived from exact retained challenge and raw
     * data. Matching these fields is structural consistency, not evidence verification.
     */
    @JvmStatic
    fun possessionMessageBytes(challenge: Challenge, raw: RawOriginal): ByteArray {
        requireRawBinding(challenge, raw)
        return POSSESSION_DOMAIN + challenge.attestationChallengeBytes() +
            chainDigestBytes(raw.certificateChainDerBytes()) + raw.appPublicKeySec1Bytes()
    }

    /**
     * Strict DER structural validation using the current SDK decoder. Preserve the original DER,
     * including a valid high-S encoding: authentication transport never converts it to Wallet wire.
     * This method does not verify a signature.
     */
    @JvmStatic
    fun canonicalPossessionDerBytes(original: ByteArray): ByteArray {
        val der = boundedCopy(original, 72)
        KagemushaP256Codec.rawLowSFromStrictDer(der)
        return der
    }

    /** Exact, unpadded Base64URL application Integrity requestHash; no decoded Google verdict. */
    @JvmStatic
    fun integrityRequestHashText(
        challenge: Challenge,
        raw: RawOriginal,
        possessionMessage: ByteArray,
        possessionSignatureDer: ByteArray,
    ): String {
        val message = boundedCopy(possessionMessage, 1024)
        val der = canonicalPossessionDerBytes(possessionSignatureDer)
        require(message.contentEquals(possessionMessageBytes(challenge, raw))) {
            "first-device auth possession original does not match retained data"
        }
        val framed = ByteArrayOutputStream().apply {
            write(INTEGRITY_DOMAIN)
            for (item in listOf(challenge.transcriptBytes(), raw.originalBytes(), message, der)) {
                write(u32le(item.size))
                write(item)
            }
        }.toByteArray()
        return Base64.getUrlEncoder().withoutPadding().encodeToString(sha(framed))
    }

    /** Pure finish HTTP body. Native independently verifies the actual key, signature and token. */
    @JvmStatic
    fun finishRequest(
        challenge: Challenge,
        raw: RawOriginal,
        possessionMessage: ByteArray,
        possessionSignatureDer: ByteArray,
        playIntegrityToken: String,
    ): HttpRequestData {
        val message = boundedCopy(possessionMessage, 1024)
        val der = canonicalPossessionDerBytes(possessionSignatureDer)
        integrityRequestHashText(challenge, raw, message, der)
        graphic(playIntegrityToken, 65536)
        return request(
            "/finish", challenge.operationId, linkedMapOf(
                "challenge_original_base64" to Json.of(standard(challenge.originalBytes())),
                "raw_verifier_original_base64" to Json.of(standard(raw.originalBytes())),
                "possession_message_original_base64" to Json.of(standard(message)),
                "possession_signature_der_base64" to Json.of(standard(der)),
                "play_integrity_token" to Json.of(playIntegrityToken),
            ),
        )
    }

    /** Mandatory explicit null supplies no finish result or authority. */
    @JvmStatic
    fun parseFinishResponse(body: ByteArray): FinishOriginal? =
        nullableOriginal(body, "result_original_base64")?.let { FinishOriginal.parse(it) }

    /** Recovery preserves original challenge and phase while supplying fresh Google authorization. */
    @JvmStatic
    fun recoveryRequest(
        challenge: Challenge,
        googleIdToken: String,
        phase: RecoveryPhase,
    ): HttpRequestData {
        graphic(googleIdToken, 16384)
        return request(
            "/recover", challenge.operationId, linkedMapOf(
                "challenge_original_base64" to Json.of(standard(challenge.originalBytes())),
                "google_id_token" to Json.of(googleIdToken), "phase" to Json.of(phase.wireName),
            ),
        )
    }

    /**
     * Return the exact original for the requested phase, or explicit null. Callers parse the
     * appropriate data type separately; this function creates no authentication session.
     */
    @JvmStatic
    fun parseRecoveryResponse(body: ByteArray): ByteArray? =
        nullableOriginal(body, "recovered_original_base64")

    /**
     * Check prepare response consistency with exact submitted caller data. This cannot verify the
     * configured policy or Google owner; those remain the private Core verifier's responsibility.
     */
    @JvmStatic
    fun requirePrepareBinding(
        challenge: Challenge,
        operationId: String,
        clientNonce: String,
        alias: String,
        originalGoogleIdToken: String,
    ) {
        digest(operationId)
        digest(clientNonce)
        graphic(alias, 256)
        graphic(originalGoogleIdToken, 16384)
        require(challenge.operationId == operationId && challenge.clientNonce == clientNonce &&
            digest(challenge.aliasDigest).contentEquals(sha(ascii(alias))) &&
            digest(challenge.googleTokenOriginalSha256).contentEquals(sha(ascii(originalGoogleIdToken)))) {
            "first-device auth prepare response does not match submitted data"
        }
    }

    /** Structural retained-challenge binding only, without any attestation or hardware verdict. */
    @JvmStatic
    fun requireRawBinding(challenge: Challenge, raw: RawOriginal) {
        require(digest(raw.challengeDigest).contentEquals(challenge.attestationChallengeBytes()) &&
            raw.checkedAtMs >= challenge.issuedAtMs && raw.checkedAtMs < challenge.expiresAtMs) {
            "first-device auth raw original does not match challenge data"
        }
    }

    /** Exact submitted leaf-first DER originals must remain unchanged in a raw response. */
    @JvmStatic
    fun requireRawChainBinding(raw: RawOriginal, submittedChain: List<ByteArray>) {
        val chain = copyChain(submittedChain)
        chainDigestBytes(chain)
        val returned = raw.certificateChainDerBytes()
        require(chain.size == returned.size && chain.indices.all { chain[it].contentEquals(returned[it]) }) {
            "first-device auth raw response does not match submitted certificate originals"
        }
    }

    /**
     * Check finish response consistency with the four exact retained originals and opaque token.
     * Matching historical data cannot reconstruct Core's opaque verified result or a login session.
     */
    @JvmStatic
    fun requireFinishBinding(
        challenge: Challenge,
        raw: RawOriginal,
        possessionMessage: ByteArray,
        possessionSignatureDer: ByteArray,
        opaquePlayIntegrityToken: String,
        finish: FinishOriginal,
    ) {
        val message = boundedCopy(possessionMessage, 1024)
        val der = canonicalPossessionDerBytes(possessionSignatureDer)
        val requestHash = Base64.getUrlDecoder().decode(integrityRequestHashText(challenge, raw, message, der))
        graphic(opaquePlayIntegrityToken, 65536)
        require(finish.configSha256 == raw.configSha256 &&
            digest(finish.challengeDigest).contentEquals(challenge.attestationChallengeBytes()) &&
            digest(finish.rawVerifierOriginalSha256).contentEquals(sha(raw.originalBytes())) &&
            digest(finish.possessionMessageSha256).contentEquals(sha(message)) &&
            digest(finish.possessionDerSha256).contentEquals(sha(der)) &&
            digest(finish.tokenOriginalSha256).contentEquals(sha(ascii(opaquePlayIntegrityToken))) &&
            digest(finish.integrityRequestHash).contentEquals(requestHash) &&
            finish.verifiedAtMs >= raw.checkedAtMs && finish.verifiedAtMs < challenge.expiresAtMs) {
            "first-device auth finish response does not match submitted data"
        }
    }

    private fun nullableOriginal(body: ByteArray, field: String): ByteArray? {
        val obj = objectFrom(boundedCopy(body, MAX_HTTP_BODY_BYTES), setOf(field))
        return when (val value = obj[field]) {
            JsonNull -> null
            is JsonString -> binary(value.value, MAX_ORIGINAL_BYTES)
            else -> throw IllegalArgumentException("first-device auth response requires an original or explicit null")
        }
    }

    private fun request(suffix: String, operation: String, members: Map<String, Json>): HttpRequestData =
        HttpRequestData(ROUTE + suffix, operation, Json.obj(members).toJsonBytes())

    private fun objectFrom(bytes: ByteArray, fields: Set<String>): JsonObject {
        val obj = Json.parse(bytes) as? JsonObject
            ?: throw IllegalArgumentException("first-device auth JSON must be an object")
        require(obj.keys == fields) { "first-device auth JSON fields do not match the closed contract" }
        return obj
    }

    private fun schema(obj: JsonObject, expected: String) {
        require(string(obj, "schema") == expected && smallInteger(obj, "version") == 1) {
            "first-device auth schema or version is invalid"
        }
    }

    private fun string(obj: JsonObject, field: String): String =
        (obj[field] as? JsonString)?.value
            ?: throw IllegalArgumentException("first-device auth field must be a string")

    private fun u64(obj: JsonObject, field: String): BigInteger {
        val number = obj[field] as? JsonNumber
            ?: throw IllegalArgumentException("first-device auth time must be an unsigned integer")
        require(number.text.isNotEmpty() && number.text.length <= 20 && number.text.all { it in '0'..'9' }) {
            "first-device auth integer must use canonical unsigned decimal"
        }
        val value = BigInteger(number.text)
        require(value.signum() >= 0 && value <= MAX_U64) {
            "first-device auth integer exceeds unsigned64"
        }
        return value
    }

    private fun smallInteger(obj: JsonObject, field: String): Int {
        val value = u64(obj, field)
        require(value <= BigInteger.valueOf(Int.MAX_VALUE.toLong())) {
            "first-device auth integer exceeds its field width"
        }
        return value.toInt()
    }

    private fun digestText(obj: JsonObject, field: String): String =
        string(obj, field).also { digest(it) }

    private fun digest(value: String): ByteArray {
        require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "first-device auth digest must be nonzero lowercase hex32"
        }
        return ByteArray(32) { index ->
            ((Character.digit(value[index * 2], 16) shl 4) or
                Character.digit(value[index * 2 + 1], 16)).toByte()
        }.also { require(it.any { byte -> byte.toInt() != 0 }) { "first-device auth digest is zero" } }
    }

    private fun binary(value: String, maximum: Int, exact: Int? = null): ByteArray {
        require(value.isNotEmpty() && value.length <= ((maximum + 2) / 3) * 4) {
            "first-device auth binary original is out of bounds"
        }
        val raw = Base64.getDecoder().decode(value)
        require(raw.isNotEmpty() && raw.size <= maximum && (exact == null || raw.size == exact) &&
            standard(raw) == value) {
            "first-device auth binary original must use canonical padded standard Base64"
        }
        return raw
    }

    private fun chainFrom(obj: JsonObject, field: String): List<ByteArray> {
        val array = obj[field] as? JsonArray
            ?: throw IllegalArgumentException("first-device auth chain must be an array")
        require(array.items.size in 2..8) { "first-device auth certificate count is out of bounds" }
        return array.items.map { item ->
            val text = (item as? JsonString)?.value
                ?: throw IllegalArgumentException("first-device auth chain element must be a string")
            binary(text, MAX_CERTIFICATE_BYTES)
        }
    }

    private fun encodedChain(chain: List<ByteArray>): JsonArray =
        Json.array(chain.map { Json.of(standard(it)) })

    private fun copyChain(chain: List<ByteArray>): List<ByteArray> {
        require(chain.size in 2..8) { "first-device auth certificate count is out of bounds" }
        return chain.map { boundedCopy(it, MAX_CERTIFICATE_BYTES) }
    }

    private fun boundedCopy(original: ByteArray, maximum: Int): ByteArray {
        require(original.isNotEmpty() && original.size <= maximum) { "first-device auth original is out of bounds" }
        return original.copyOf()
    }

    private fun graphic(value: String, maximum: Int) {
        require(value.isNotEmpty() && value.length <= maximum && value.all { it in '!'..'~' }) {
            "first-device auth text original is out of bounds or is not ASCII graphic"
        }
    }

    private fun sha(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)

    private fun standard(value: ByteArray): String = Base64.getEncoder().encodeToString(value)

    private fun ascii(value: String): ByteArray = value.toByteArray(Charsets.US_ASCII)

    private fun u32le(value: Int): ByteArray =
        ByteArray(4) { index -> (value ushr (index * 8)).toByte() }

    private fun u64le(value: BigInteger): ByteArray =
        ByteArray(8) { index -> value.shiftRight(index * 8).toByte() }
}
