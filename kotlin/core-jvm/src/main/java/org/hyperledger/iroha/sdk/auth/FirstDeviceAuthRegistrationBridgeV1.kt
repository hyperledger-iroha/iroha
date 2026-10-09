// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.auth

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.Base64
import java.util.UUID
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol

/** Closed DATA for hardware-auth admission to the existing Ed-owner registration producer.
 * Parsing verifies original consistency only: Core independently reacquires its opaque verifier,
 * verifies the owner Ed proof and reserves registration in its existing Native atomic CAS. No
 * frame, declared DATA incarnation or returned PENDING status is a login or Wallet capability.
 */
object FirstDeviceAuthRegistrationBridgeV1 {
    const val FRAME_PATH = "/v1/kagemusha/hardware-evidence/first-device/registration-frame"
    const val REGISTER_PATH = "/v1/kagemusha/hardware-evidence/first-device/register"
    const val MAX_FRAME_BYTES = 4096
    private val DOMAIN = "BPNG/FIRST-DEVICE/RETAIL-REGISTRATION/V1\u0000".toByteArray(Charsets.US_ASCII)
    private val MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    /** Exact existing producer request; its Ed signature is still verified by that producer/Core. */
    class RegistrationOriginal private constructor(original: ByteArray, val accountId: String,
        val requestId: String, val issuedAtMs: BigInteger, val expiresAtMs: BigInteger) {
        private val retained = original.copyOf()
        fun originalBytes(): ByteArray = retained.copyOf()

        companion object {
            @JvmStatic fun parse(original: ByteArray, originalAccountId: String): RegistrationOriginal {
                val bytes = bounded(original, 65536)
                val obj = closed(bytes, setOf("locale", "fi_id", "dataspace_id", "user", "device", "keys",
                    "activation_grant_sha256", "owner_proof"))
                require(text(obj, "fi_id") == "mibank.bpng" && text(obj, "dataspace_id") == "bpng")
                boundedText(obj, "locale", 16)
                hex32(text(obj, "activation_grant_sha256"))
                val keys = child(obj, "keys", setOf("online_public_key"))
                val publicKey = text(keys, "online_public_key")
                require(publicKey.matches(Regex("ed0120[0-9A-F]{64}")))
                val account = AccountAddress.fromAccount(hex(publicKey.substring(6)), "ed25519").toI105Default()
                require(account == originalAccountId) { "Registration differs from the existing Ed account" }
                val proof = child(obj, "owner_proof", setOf("request_id", "issued_at_ms", "expires_at_ms", "signature_base64url"))
                val requestId = uuid(text(proof, "request_id"), version4 = true)
                val issued = integer(proof, "issued_at_ms")
                val expires = integer(proof, "expires_at_ms")
                require(issued.signum() > 0 && expires > issued && expires <= BigInteger.valueOf(Long.MAX_VALUE) &&
                    expires.subtract(issued) <= BigInteger.valueOf(300000))
                val ownerSignature = text(proof, "signature_base64url")
                val signature = Base64.getUrlDecoder().decode(ownerSignature)
                require(signature.size == 64 && Base64.getUrlEncoder().withoutPadding().encodeToString(signature) == ownerSignature)
                // These exact bytes, including signup alias/address/device, are hashed into the frame.
                val user = child(obj, "user", setOf("first_name", "last_name", "email", "phone", "alias", "address"))
                for ((field, limit) in listOf("first_name" to 128, "last_name" to 128, "email" to 320, "phone" to 32, "alias" to 256))
                    boundedText(user, field, limit)
                val address = child(user, "address", setOf("line1", "city", "postal_code", "country"))
                for ((field, limit) in listOf("line1" to 256, "city" to 128, "postal_code" to 32, "country" to 2))
                    boundedText(address, field, limit)
                require(text(address, "country") == "PG")
                val device = child(obj, "device", setOf("device_id", "platform", "app_version"))
                boundedText(device, "device_id", 256); boundedText(device, "app_version", 64)
                require(text(device, "platform") == "android")
                return RegistrationOriginal(bytes, account, requestId, issued, expires)
            }
        }
    }

    /** Retained completed auth and existing registration DATA plus the genuine caller's runtime.
     * The app must guard their released source/owner and encrypted original records around use.
     */
    class Originals private constructor(val registration: RegistrationOriginal, val challenge: Protocol.Challenge,
        val raw: Protocol.RawOriginal, val finish: Protocol.FinishOriginal, val googleIdToken: String,
        val networkId: NetworkId, val physicalDataspaceId: BigInteger, val laneId: Long) {
        companion object {
            @JvmStatic fun retain(registration: RegistrationOriginal, challenge: Protocol.Challenge,
                raw: Protocol.RawOriginal, possessionMessage: ByteArray, possessionDer: ByteArray,
                integrityToken: String, finish: Protocol.FinishOriginal, originalGoogleIdToken: String,
                networkId: NetworkId, physicalDataspaceId: BigInteger, laneId: Long): Originals {
                Protocol.requireRawBinding(challenge, raw)
                Protocol.requireFinishBinding(challenge, raw, possessionMessage, possessionDer, integrityToken, finish)
                require(Protocol.googleIdTokenOriginalSha256(originalGoogleIdToken) == challenge.googleTokenOriginalSha256)
                require(physicalDataspaceId.signum() > 0 && physicalDataspaceId <= MAX_U64 && laneId in 0..0xffffffffL)
                return Originals(registration, challenge, raw, finish, originalGoogleIdToken, networkId, physicalDataspaceId, laneId)
            }
        }
    }

    /** Only the declared, bounded registration purpose can enter the auth-key signer. */
    class FrameOriginal private constructor(original: ByteArray, responseOriginal: ByteArray, internal val originals: Originals,
        val declaredIncarnation: String) {
        private val retained = original.copyOf()
        private val retainedResponse = bounded(responseOriginal, 8192)
        val registrationRequestId: String get() = originals.registration.requestId
        fun originalBytes(): ByteArray = retained.copyOf()
        fun responseOriginalBytes(): ByteArray = retainedResponse.copyOf()

        /** Match full attested key originals and transcript, independently of either signup alias
         * or the random Keystore slot selected by Native after the prepare correlation alias.
         */
        fun requireKeyBinding(challenge: Protocol.Challenge, publicKey: ByteArray, chain: List<ByteArray>) {
            require(challenge == originals.challenge)
            Protocol.requireRawBinding(challenge, originals.raw)
            Protocol.requireRawChainBinding(originals.raw, chain)
            require(originals.raw.appPublicKeySec1Bytes().contentEquals(publicKey))
        }

        companion object {
            @JvmStatic fun parseResponse(responseOriginal: ByteArray, originals: Originals): FrameOriginal {
                val obj = closed(bounded(responseOriginal, 8192), setOf("frame_original_base64"))
                val encoded = text(obj, "frame_original_base64")
                require(encoded.length <= ((MAX_FRAME_BYTES + 2) / 3) * 4)
                val bytes = bounded(Base64.getDecoder().decode(encoded), MAX_FRAME_BYTES)
                require(Base64.getEncoder().encodeToString(bytes) == encoded)
                val reader = Reader(bytes)
                reader.expect(DOMAIN)
                require(reader.uint(2) == BigInteger.ONE)
                require(reader.part() == originals.networkId.literal)
                require(reader.part() == "mibank.bpng")
                require(reader.uint(8) == originals.physicalDataspaceId && reader.uint(4) == BigInteger.valueOf(originals.laneId))
                require(reader.part() == "mibank-core" && reader.part() == "mibank-core")
                val incarnation = uuid(reader.part()) // Server-declared DATA, never app-verified Native authority.
                require(reader.part() == "POST" && reader.part() == "/mibank$REGISTER_PATH" && reader.part() == "wallet-android")
                require(reader.part() == originals.registration.requestId && reader.part() == originals.registration.accountId)
                for (expected in listOf(hex32(originals.challenge.operationId), hex32(originals.challenge.policySha256),
                    hex32(originals.finish.configSha256), hex32(originals.challenge.googleOwnerBinding),
                    hex32(originals.challenge.googleTokenOriginalSha256), sha(originals.challenge.originalBytes()),
                    sha(originals.finish.originalBytes()), sha(originals.registration.originalBytes()))) reader.expect(expected)
                require(reader.uint(1) == BigInteger.ZERO) { "Registration has no selected HTTP request key" }
                require(reader.uint(8) == originals.registration.issuedAtMs && reader.uint(8) == originals.registration.expiresAtMs)
                require(reader.finished()) { "Registration frame has trailing data" }
                return FrameOriginal(bytes, responseOriginal, originals, incarnation)
            }
        }
    }

    /** Exact original bodies and UUID correlation for the two closed anonymous bootstrap routes. */
    class RequestData private constructor(val path: String, val registrationRequestId: String, original: ByteArray) {
        private val retained = bounded(original, Protocol.MAX_HTTP_BODY_BYTES)
        fun bodyBytes(): ByteArray = retained.copyOf()
        companion object {
            internal fun from(path: String, originals: Originals, der: ByteArray?): RequestData {
                val values = linkedMapOf<String, Json>(
                    "registration_request_original_base64" to Json.of(base64(originals.registration.originalBytes())),
                    "challenge_original_base64" to Json.of(base64(originals.challenge.originalBytes())),
                    "finish_verifier_original_base64" to Json.of(base64(originals.finish.originalBytes())),
                    "google_id_token" to Json.of(originals.googleIdToken))
                if (der != null) values["authentication_signature_der_base64"] = Json.of(base64(Protocol.canonicalPossessionDerBytes(der)))
                return RequestData(path, originals.registration.requestId, Json.obj(values).toJsonBytes())
            }
        }
    }

    @JvmStatic fun frameRequest(originals: Originals): RequestData = RequestData.from(FRAME_PATH, originals, null)
    @JvmStatic fun registerRequest(frame: FrameOriginal, signatureDer: ByteArray): RequestData =
        RequestData.from(REGISTER_PATH, frame.originals, signatureDer)

    /** Return matching existing PENDING registration DATA only, without activation or session. */
    @JvmStatic fun requirePendingResponse(responseOriginal: ByteArray, registration: RegistrationOriginal) {
        val obj = closed(bounded(responseOriginal, 8192), setOf("registration_id", "account_id", "status"))
        require(text(obj, "registration_id") == registration.requestId && text(obj, "account_id") == registration.accountId &&
            text(obj, "status") == "PENDING")
    }

    @JvmStatic fun requirePendingFrameResponse(responseOriginal: ByteArray, frame: FrameOriginal) =
        requirePendingResponse(responseOriginal, frame.originals.registration)

    private class Reader(private val bytes: ByteArray) {
        private var offset = 0
        fun take(size: Int): ByteArray {
            require(size >= 0 && size <= bytes.size - offset) { "Registration frame is truncated" }
            return bytes.copyOfRange(offset, offset + size).also { offset += size }
        }
        fun expect(expected: ByteArray) { require(take(expected.size).contentEquals(expected)) { "Registration frame binding differs" } }
        fun uint(size: Int) = BigInteger(1, take(size))
        fun part(): String {
            val size = uint(4)
            require(size <= BigInteger.valueOf(MAX_FRAME_BYTES.toLong()))
            val value = take(size.toInt())
            return Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(value)).toString()
        }
        fun finished() = offset == bytes.size
    }

    private fun closed(bytes: ByteArray, fields: Set<String>): JsonObject = (Json.parse(bytes) as? JsonObject
        ?: throw IllegalArgumentException("Registration original must be a JSON object"))
        .also { require(it.keys == fields) { "Registration original fields differ from the closed contract" } }
    private fun child(obj: JsonObject, field: String, fields: Set<String>): JsonObject = (obj[field] as? JsonObject
        ?: throw IllegalArgumentException("Registration original requires object $field"))
        .also { require(it.keys == fields) }
    private fun text(obj: JsonObject, field: String): String = (obj[field] as? JsonString)?.value
        ?: throw IllegalArgumentException("Registration original requires string $field")
    private fun boundedText(obj: JsonObject, field: String, maximum: Int): String = text(obj, field).also {
        require(it.isNotEmpty() && it.toByteArray(Charsets.UTF_8).size <= maximum)
    }
    private fun integer(obj: JsonObject, field: String): BigInteger {
        val value = (obj[field] as? JsonNumber)?.text ?: throw IllegalArgumentException("Registration time must be an integer")
        require(value.matches(Regex("0|[1-9][0-9]{0,19}")))
        return BigInteger(value).also { require(it <= MAX_U64) }
    }
    private fun uuid(value: String, version4: Boolean = false): String {
        val parsed = UUID.fromString(value)
        require(parsed.toString() == value && (parsed.mostSignificantBits != 0L || parsed.leastSignificantBits != 0L) &&
            (!version4 || parsed.version() == 4 && parsed.variant() == 2))
        return value
    }
    private fun hex(value: String): ByteArray = ByteArray(value.length / 2) { value.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    private fun hex32(value: String) = hex(value.also { require(it.matches(Regex("[0-9a-f]{64}")) && it.any { c -> c != '0' }) })
    private fun bounded(bytes: ByteArray, maximum: Int) = bytes.copyOf().also { require(it.isNotEmpty() && it.size <= maximum) }
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
    private fun base64(bytes: ByteArray) = Base64.getEncoder().encodeToString(bytes)
}
