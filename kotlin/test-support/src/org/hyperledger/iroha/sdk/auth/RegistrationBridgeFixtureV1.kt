package org.hyperledger.iroha.sdk.auth

import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthRegistrationBridgeV1 as Bridge

/** Synthetic public protocol DATA only. No Google, hardware, registration or Native auth verdict. */
object RegistrationBridgeFixtureV1 {
    const val GOOGLE = "synthetic.google.original"
    const val INTEGRITY = "synthetic.integrity.original"
    const val REQUEST_ID = "12000000-0000-4000-8000-000000000001"
    const val INCARNATION = "13000000-0000-4000-8000-000000000002"
    const val ED = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    val DER: ByteArray get() = byteArrayOf(0x30, 6, 2, 1, 1, 2, 1, 1)
    fun digest(byte: Int) = "%02x".format(byte).repeat(32)
    fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
    fun shaHex(bytes: ByteArray) = sha(bytes).joinToString("") { "%02x".format(it.toInt() and 255) }
    fun b64(bytes: ByteArray) = Base64.getEncoder().encodeToString(bytes)
    fun account() = AccountAddress.fromAccount(ByteArray(32) { ED.substring(it * 2, it * 2 + 2).toInt(16).toByte() }, "ed25519").toI105Default()
    fun network() = NetworkId.fromBytes(ByteArray(32) { if (it == 31) 1 else 9 })
    fun challenge() = Protocol.Challenge.parse(Json.obj(linkedMapOf(
        "schema" to Json.of("bpng.first-device-auth-challenge.v1"), "version" to Json.of(1),
        "policy_sha256" to Json.of(digest(1)), "operation_id" to Json.of(digest(2)),
        "client_nonce" to Json.of(digest(3)), "server_nonce" to Json.of(digest(4)),
        "alias_digest" to Json.of(shaHex("auth-correlation-original".toByteArray())),
        "google_owner_binding" to Json.of(digest(6)),
        "google_token_original_sha256" to Json.of(Protocol.googleIdTokenOriginalSha256(GOOGLE)),
        "issued_at_ms" to Json.of(1000), "expires_at_ms" to Json.of(2000))).toJsonBytes())
    fun raw(c: Protocol.Challenge) = Protocol.RawOriginal.parse(Json.obj(linkedMapOf(
        "schema" to Json.of("bpng.first-device-auth-raw.v1"), "version" to Json.of(1),
        "config_sha256" to Json.of(digest(8)), "challenge_digest" to Json.of(shaHex(c.transcriptBytes())),
        "raw_request_sha256" to Json.of(digest(9)),
        "app_public_key_sec1_base64" to Json.of(b64(hex("046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"))),
        "security_level" to Json.of(1), "checked_at_ms" to Json.of(1500),
        "original_chain_base64" to Json.array(listOf(Json.of("AQ=="), Json.of("Ag=="))))).toJsonBytes())
    fun registrationBytes() = Json.obj(linkedMapOf(
        "locale" to Json.of("en"), "fi_id" to Json.of("mibank.bpng"), "dataspace_id" to Json.of("bpng"),
        "user" to Json.obj(linkedMapOf("first_name" to Json.of("Test"), "last_name" to Json.of("DATA"),
            "email" to Json.of("test@example.invalid"), "phone" to Json.of("+000000"),
            "alias" to Json.of("signup-alias"), "address" to Json.obj(linkedMapOf(
                "line1" to Json.of("Test"), "city" to Json.of("Test"), "postal_code" to Json.of("000"), "country" to Json.of("PG"))))),
        "device" to Json.obj(linkedMapOf("device_id" to Json.of("test-device"), "platform" to Json.of("android"), "app_version" to Json.of("test"))),
        "keys" to Json.obj(mapOf("online_public_key" to Json.of("ed0120" + ED.uppercase()))),
        "activation_grant_sha256" to Json.of(digest(20)),
        "owner_proof" to Json.obj(linkedMapOf("request_id" to Json.of(REQUEST_ID), "issued_at_ms" to Json.of(1000),
            "expires_at_ms" to Json.of(2000), "signature_base64url" to Json.of(Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(64) { 1 })))))).toJsonBytes()
    fun originals(c: Protocol.Challenge = challenge(), r: Protocol.RawOriginal = raw(c), der: ByteArray = DER): Bridge.Originals {
        val message = Protocol.possessionMessageBytes(c, r)
        val finish = Protocol.FinishOriginal.parse(Json.obj(linkedMapOf(
            "schema" to Json.of("bpng.first-device-auth-finish.v1"), "version" to Json.of(1),
            "config_sha256" to Json.of(r.configSha256), "challenge_digest" to Json.of(shaHex(c.transcriptBytes())),
            "raw_verifier_original_sha256" to Json.of(shaHex(r.originalBytes())),
            "possession_message_sha256" to Json.of(shaHex(message)), "possession_der_sha256" to Json.of(shaHex(der)),
            "token_original_sha256" to Json.of(shaHex(INTEGRITY.toByteArray())),
            "integrity_request_hash" to Json.of(Base64.getUrlDecoder().decode(Protocol.integrityRequestHashText(c, r, message, der)).joinToString("") { "%02x".format(it.toInt() and 255) }),
            "verified_at_ms" to Json.of(1600), "google_response_original_base64" to Json.of("e30="))).toJsonBytes())
        return Bridge.Originals.retain(Bridge.RegistrationOriginal.parse(registrationBytes(), account()), c, r,
            message, der, INTEGRITY, finish, GOOGLE, network(), BigInteger("72623859790382856"), 0x01020304L)
    }
    /** Independent DataOutputStream reference implements the public BE wire ordering. */
    fun frame(o: Bridge.Originals, incarnation: String = INCARNATION): ByteArray {
        val bytes = ByteArrayOutputStream()
        DataOutputStream(bytes).use { out ->
            fun part(s: String) { val b = s.toByteArray(Charsets.UTF_8); out.writeInt(b.size); out.write(b) }
            out.write("BPNG/FIRST-DEVICE/RETAIL-REGISTRATION/V1\u0000".toByteArray(Charsets.US_ASCII))
            out.writeShort(1); part(o.networkId.literal); part("mibank.bpng")
            out.writeLong(o.physicalDataspaceId.toLong()); out.writeInt(o.laneId.toInt())
            part("mibank-core"); part("mibank-core"); part(incarnation); part("POST")
            part("/mibank/v1/kagemusha/hardware-evidence/first-device/register"); part("wallet-android")
            part(o.registration.requestId); part(o.registration.accountId)
            listOf(hex(o.challenge.operationId), hex(o.challenge.policySha256), hex(o.finish.configSha256),
                hex(o.challenge.googleOwnerBinding), hex(o.challenge.googleTokenOriginalSha256),
                sha(o.challenge.originalBytes()), sha(o.finish.originalBytes()), sha(o.registration.originalBytes())).forEach(out::write)
            out.writeByte(0); out.writeLong(o.registration.issuedAtMs.toLong()); out.writeLong(o.registration.expiresAtMs.toLong())
        }
        return bytes.toByteArray()
    }
    fun response(frame: ByteArray) = Json.obj(mapOf("frame_original_base64" to Json.of(b64(frame)))).toJsonBytes()
    fun hex(s: String) = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}
