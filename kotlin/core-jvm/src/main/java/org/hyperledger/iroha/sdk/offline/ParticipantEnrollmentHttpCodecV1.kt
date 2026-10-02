// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.net.URI
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Locale
import org.bouncycastle.math.ec.rfc8032.Ed25519
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission

/** Exact public enrollment message/header grammar. No Native, FI or current-wallet authority.
 * A decoded signature is still untrusted customer/current-read evidence. Dispatch must remain
 * under the actual finite Native owner; copied bytes and these types cannot renew that owner.
 * Authorization and returned headers are runtime originals and must never be logged or persisted.
 */
object ParticipantEnrollmentHttpCodecV1 {
    enum class Operation(val tag: Int, val pathSuffix: String) {
        PREPARE(1, "/v1/kagemusha/enrollment/ordinary/prepare"),
        RAW_ATTESTATION(2, "/v1/kagemusha/enrollment/ordinary/raw-attestation"),
        CERTIFICATE(3, "/v1/kagemusha/enrollment/ordinary/certificate"),
    }

    /** Primitive independently selected context, never a verified owner or decoded authority. */
    class Context internal constructor(
        val networkId: NetworkId,
        val authenticationNamespace: String,
        val actorId: String,
        val operation: Operation,
        val target: URI,
        internal val origin: String,
        internal val path: String,
    )

    /** Exact unsigned original. Public shape/membership checks grant no certified current cut. */
    class Request internal constructor(
        val context: Context,
        val signatoryI105: String,
        val walletI105: String,
        val requestId: String,
        val idempotencyKey: String,
        val timestampMs: BigInteger,
        val nonce: String,
        internal val signatory: AccountAddress,
        internal val wallet: AccountAddress,
        body: ByteArray,
        sessionSha256: ByteArray,
    ) {
        private val body = body.copyOf()
        private val session = sessionSha256.copyOf()
        fun originalBody(): ByteArray = body.copyOf()
        fun sessionSha256(): ByteArray = session.copyOf()
        fun signingMessage(): ByteArray = message(this)
    }

    /** One flattened actual HTTP value; use a list so duplicates cannot disappear in a map. */
    class Header(val name: String, value: ByteArray) {
        private val original: ByteArray
        init {
            // Preserve unrelated-header behavior; enforce the existing cap before known-value copies.
            require(!names.contains(name.lowercase(Locale.ROOT)) || value.size <= 8192)
            original = value.copyOf()
        }
        fun originalValue(): ByteArray = original.copyOf()
        internal fun enrollmentValue(): ByteArray {
            require(original.size <= 8192)
            return original.copyOf()
        }
    }

    /** Signature-checked original only; no constructor for a Native/FI verified request exists. */
    class ReceivedOriginal internal constructor(val request: Request, signature: ByteArray) {
        private val signature = signature.copyOf()
        fun originalSignature(): ByteArray = signature.copyOf()
    }

    private val names = listOf("x-iroha-enrollment-contract", "x-iroha-enrollment-signatory",
        "x-iroha-enrollment-wallet", "x-iroha-enrollment-request-id", "idempotency-key",
        "x-iroha-enrollment-timestamp", "x-iroha-enrollment-nonce", "x-iroha-enrollment-signature",
        "authorization")
    private val maximumU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val skew = BigInteger.valueOf(30_000)
    private val outerDomain = "iroha.participant.ordinary-enrollment-request.v1\u0000".toByteArray(StandardCharsets.US_ASCII)
    private val networkDomain = "iroha.app.request.network.v1\u0000".toByteArray(StandardCharsets.US_ASCII)

    @JvmStatic fun headerNames(): List<String> = names.toList()

    /** [target] is the canonical serialized Rust HTTPS target, supplied by the actual mount.
     * Noncanonical spellings are refused, never normalized or reconstructed from offered headers.
     */
    @JvmStatic fun context(networkId: NetworkId, authenticationNamespace: String, actorId: String,
        operation: Operation, target: URI): Context {
        visible(authenticationNamespace, 256); visible(actorId, 256)
        val host = target.host ?: error("Missing exact enrollment HTTPS host")
        val port = target.port
        val authority = host + if (port == -1) "" else ":$port"
        require(target.scheme == "https" && target.rawUserInfo == null && target.rawQuery == null &&
            target.rawFragment == null && target.rawAuthority == authority &&
            host == host.lowercase(Locale.ROOT) && port != 443 && port in -1..65535 &&
            target.toString() == target.toASCIIString() && target.normalize() == target)
        val path = target.rawPath ?: error("Missing original enrollment path")
        require(path.startsWith("/") && path.length <= 64 * 1024 && path.endsWith(operation.pathSuffix))
        require(path.split('/').none { val segment = it.lowercase(Locale.ROOT).replace("%2e", "."); segment == "." || segment == ".." })
        return Context(networkId, authenticationNamespace, actorId, operation, target, "https://$authority", path)
    }

    @JvmStatic fun originalRequest(context: Context, signatoryI105: String, walletI105: String,
        requestId: String, idempotencyKey: String, timestampMs: BigInteger, nonce: String,
        originalBody: ByteArray, sessionSha256: ByteArray): Request {
        require(originalBody.isNotEmpty() && originalBody.size <= 256 * 1024 && sessionSha256.size == 32)
        val body = originalBody.copyOf(); val session = sessionSha256.copyOf()
        visible(requestId, 128); visible(idempotencyKey, 256); canonicalNonce(nonce)
        require(timestampMs > skew && timestampMs <= maximumU64.subtract(skew))
        require(body.isNotEmpty() && body.size <= 256 * 1024 &&
            session.size == 32 && session.any { it != 0.toByte() })
        val s = account(signatoryI105); val w = account(walletI105)
        val key = s.singleKeyPayload() ?: error("Enrollment S is not a single Ed key")
        val policy = w.multisigPolicyPayload() ?: error("Enrollment W is not native multisig")
        val members = policy.members
        require(key.curveId == 1 && key.publicKey.size == 32 && policy.threshold == 1 &&
            members.size == 1 && members[0].weight == 1 && members[0].curveId == 1 &&
            members[0].publicKey.contentEquals(key.publicKey) && !s.canonicalBytes.contentEquals(w.canonicalBytes))
        return Request(context, signatoryI105, walletI105, requestId, idempotencyKey, timestampMs,
            nonce, s, w, body, session)
    }

    @JvmStatic fun authorizationDigest(authorization: ByteArray): ByteArray {
        require(authorization.isNotEmpty() && authorization.size <= 8192)
        val snapshot = authorization.copyOf()
        require(snapshot.isNotEmpty() && snapshot.size <= 8192 &&
            snapshot.all { (it.toInt() and 255) in 0x20..0x7e })
        return sha(snapshot)
    }

    /** Retains a real existing raw Ed64; does not invoke any signer or construct Native custody. */
    @JvmStatic fun encodeHeaders(request: Request, originalSignature: ByteArray,
        authorization: ByteArray): List<Header> {
        require(originalSignature.size == 64 && authorization.isNotEmpty() && authorization.size <= 8192)
        val signature = originalSignature.copyOf(); val session = authorization.copyOf()
        require(MessageDigest.isEqual(authorizationDigest(session), request.sessionSha256()))
        verify(request, signature)
        val values = listOf("1", request.signatoryI105, request.walletI105, request.requestId,
            request.idempotencyKey, request.timestampMs.toString(), request.nonce, hex(signature))
        return values.mapIndexed { i, value -> Header(names[i], value.toByteArray(StandardCharsets.UTF_8)) } +
            Header("authorization", session)
    }

    /** Capture every actual value before generic bearer `.get`; current FI admission remains separate. */
    @JvmStatic fun decode(context: Context, actualMethod: String, actualRequestTarget: String,
        originalBody: ByteArray, headers: Iterable<Header>): ReceivedOriginal {
        require(originalBody.isNotEmpty() && originalBody.size <= 256 * 1024)
        val body = originalBody.copyOf()
        require(actualMethod == "POST" && actualRequestTarget == context.path)
        val values = arrayOfNulls<ByteArray>(9)
        for (header in headers) {
            val name = header.name.lowercase(Locale.ROOT)
            val index = names.indexOf(name)
            if (index >= 0) {
                require(values[index] == null)
                val value = header.enrollmentValue(); require(value.size <= 8192); values[index] = value
            } else require(!name.startsWith("x-iroha-enrollment-"))
        }
        fun value(i: Int): ByteArray = values[i] ?: error("Missing exact enrollment HTTP value")
        fun text(i: Int): String = utf8(value(i))
        require(text(0) == "1")
        val signature = unhex(text(7), 64)
        val timestamp = decimal(text(5))
        val request = originalRequest(context, text(1), text(2), text(3), text(4), timestamp,
            text(6), body, authorizationDigest(value(8)))
        verify(request, signature)
        return ReceivedOriginal(request, signature)
    }

    private fun message(request: Request): ByteArray {
        val c = request.context
        val body = request.originalBody()
        // Exact existing Rust network subgrammar, not a generic signing callback/header producer.
        val canonical = networkDomain + c.networkId.bytes() +
            ("POST\n${c.path}\n\n${hex(sha(body))}\n${request.timestampMs}\n${request.nonce}")
                .toByteArray(StandardCharsets.US_ASCII)
        val out = ByteArrayOutputStream(canonical.size + 2048)
        out.write(outerDomain); out.write(c.operation.tag)
        for (field in listOf(c.authenticationNamespace.toByteArray(StandardCharsets.US_ASCII),
            c.actorId.toByteArray(StandardCharsets.US_ASCII), request.signatory.canonicalHex().toByteArray(StandardCharsets.US_ASCII),
            request.wallet.canonicalHex().toByteArray(StandardCharsets.US_ASCII), c.origin.toByteArray(StandardCharsets.US_ASCII),
            request.sessionSha256(), request.requestId.toByteArray(StandardCharsets.US_ASCII),
            request.idempotencyKey.toByteArray(StandardCharsets.US_ASCII), canonical)) {
            for (shift in 0..3) out.write((field.size ushr (8 * shift)) and 255)
            out.write(field)
        }
        return out.toByteArray()
    }

    private fun verify(request: Request, signature: ByteArray) {
        require(signature.size == 64 && Ed25519PublicKeyAdmission.isValid(signature.copyOfRange(0, 32)))
        val key = request.signatory.singleKeyPayload()!!.publicKey
        val message = request.signingMessage()
        // Rust Signature::try_new/verify use raw Ed25519 over the full subject, without IrohaHash prehash.
        require(Ed25519.verify(signature, 0, key, 0, message, 0, message.size))
    }
    private fun account(raw: String): AccountAddress {
        require(raw.isNotEmpty() && raw.toByteArray(StandardCharsets.UTF_8).size <= 4096)
        val account = AccountAddress.parseEncoded(raw, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        require(account.toI105Default() == raw)
        return account
    }
    private fun visible(raw: String, maximum: Int) {
        require(raw.isNotEmpty() && raw.length <= maximum && raw.all { it.code in 0x21..0x7e })
    }
    private fun canonicalNonce(raw: String) { require(raw.length == 64 && raw.all { it in '0'..'9' || it in 'a'..'f' }) }
    private fun decimal(raw: String): BigInteger {
        require(raw.isNotEmpty() && raw.length <= 20 && raw.all { it in '0'..'9' } && (raw.length == 1 || raw[0] != '0'))
        val value = BigInteger(raw); require(value <= maximumU64 && value.toString() == raw); return value
    }
    private fun utf8(raw: ByteArray): String = StandardCharsets.UTF_8.newDecoder()
        .onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
        .decode(ByteBuffer.wrap(raw)).toString()
    private fun unhex(raw: String, bytes: Int): ByteArray {
        require(raw.length == bytes * 2 && raw.all { it in '0'..'9' || it in 'a'..'f' })
        return ByteArray(bytes) { raw.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }
    private fun hex(raw: ByteArray): String = raw.joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun sha(raw: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(raw)
}
