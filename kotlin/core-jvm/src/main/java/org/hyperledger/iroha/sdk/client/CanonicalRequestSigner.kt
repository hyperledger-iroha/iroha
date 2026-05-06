package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.Signature
import java.util.Base64
import java.util.Locale

/** Builds canonical request signatures for Torii app endpoints. */
object CanonicalRequestSigner {

    const val HEADER_ACCOUNT = "X-Iroha-Account"
    const val HEADER_SIGNATURE = "X-Iroha-Signature"
    const val HEADER_TIMESTAMP_MS = "X-Iroha-Timestamp-Ms"
    const val HEADER_NONCE = "X-Iroha-Nonce"
    const val BODY_ACCOUNT_ID = "account_id"
    const val BODY_TIMESTAMP_MS = "timestamp_ms"
    const val BODY_NONCE = "nonce"
    const val BODY_SIGNATURE_BASE64 = "signature_base64"
    const val BODY_WITNESS_BASE64 = "witness_base64"

    private val NONCE_RANDOM = SecureRandom()

    /** Canonicalise a raw query string by decoding, sorting, and re-encoding. */
    @JvmStatic
    fun canonicalQueryString(raw: String?): String {
        if (raw.isNullOrEmpty()) return ""
        val pairs = ArrayList<Pair<String, String>>()
        for (component in raw.split("&")) {
            val kv = component.split("=", limit = 2)
            val key = if (kv.isNotEmpty()) kv[0] else ""
            val value = if (kv.size > 1) kv[1] else ""
            pairs.add(urlDecode(key) to urlDecode(value))
        }
        pairs.sortWith(compareBy<Pair<String, String>> { it.first }.thenBy { it.second })
        return pairs.joinToString("&") { "${urlEncode(it.first)}=${urlEncode(it.second)}" }
    }

    /** Build canonical request bytes for signing. */
    @JvmStatic
    fun canonicalRequestMessage(method: String, uri: URI, body: ByteArray?): ByteArray {
        val query = canonicalQueryString(uri.rawQuery)
        val path = uri.rawPath ?: ""
        val bodyBytes = body ?: ByteArray(0)
        val digest: ByteArray
        try {
            digest = MessageDigest.getInstance("SHA-256").digest(bodyBytes)
        } catch (ex: Exception) {
            throw IllegalStateException("sha256 unavailable", ex)
        }
        val rendered = "${method.uppercase(Locale.ROOT)}\n$path\n$query\n${hex(digest)}"
        return rendered.toByteArray(StandardCharsets.UTF_8)
    }

    /** Build canonical request bytes plus freshness metadata for signature verification. */
    @JvmStatic
    fun canonicalRequestSignatureMessage(
        method: String,
        uri: URI,
        body: ByteArray?,
        timestampMs: Long,
        nonce: String
    ): ByteArray {
        require(nonce.isNotBlank()) { "nonce is required" }
        val rendered = String(canonicalRequestMessage(method, uri, body), StandardCharsets.UTF_8) +
            "\n$timestampMs\n$nonce"
        return rendered.toByteArray(StandardCharsets.UTF_8)
    }

    /** Build unsigned canonical JSON bytes for body-auth endpoints. */
    @JvmStatic
    fun unsignedBodyAuthJson(bodyFields: Map<String, Any?>): ByteArray {
        val unsigned = LinkedHashMap<String, Any?>(bodyFields)
        unsigned.remove(BODY_SIGNATURE_BASE64)
        unsigned.remove(BODY_WITNESS_BASE64)
        return JsonEncoder.encode(unsigned).toByteArray(StandardCharsets.UTF_8)
    }

    /** Build body-auth canonical request bytes plus freshness metadata. */
    @JvmStatic
    fun canonicalBodyAuthSignatureMessage(
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        timestampMs: Long,
        nonce: String
    ): ByteArray = canonicalRequestSignatureMessage(
        method,
        uri,
        unsignedBodyAuthJson(bodyFields),
        timestampMs,
        nonce,
    )

    /** Build the top-level fields required for single-signature body auth. */
    @JvmStatic
    fun buildBodySignatureFields(
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        accountId: String,
        privateKey: PrivateKey
    ): Map<String, Any?> =
        buildBodySignatureFields(method, uri, bodyFields, accountId, privateKey, System.currentTimeMillis(), randomNonce())

    /** Build the top-level fields required for single-signature body auth with explicit freshness metadata. */
    @JvmStatic
    fun buildBodySignatureFields(
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        accountId: String,
        privateKey: PrivateKey,
        timestampMs: Long,
        nonce: String
    ): Map<String, Any?> {
        val unsigned = bodyWithBodyAuthFreshness(bodyFields, accountId, timestampMs, nonce)
        val message = canonicalBodyAuthSignatureMessage(method, uri, unsigned, timestampMs, nonce)
        val signatureBytes = signEd25519(privateKey, message)
        return mapOf(
            BODY_ACCOUNT_ID to accountId,
            BODY_TIMESTAMP_MS to timestampMs,
            BODY_NONCE to nonce,
            BODY_SIGNATURE_BASE64 to Base64.getEncoder().encodeToString(signatureBytes),
        )
    }

    /** Return a copy of `bodyFields` carrying single-signature body auth. */
    @JvmStatic
    fun withBodySignature(
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        accountId: String,
        privateKey: PrivateKey,
        timestampMs: Long,
        nonce: String
    ): Map<String, Any?> {
        val body = LinkedHashMap<String, Any?>(bodyFields)
        body.remove(BODY_WITNESS_BASE64)
        body.putAll(buildBodySignatureFields(method, uri, body, accountId, privateKey, timestampMs, nonce))
        return body
    }

    /** Return a copy of `bodyFields` carrying a prebuilt multisig witness body auth proof. */
    @JvmStatic
    fun withBodyWitness(
        bodyFields: Map<String, Any?>,
        accountId: String,
        timestampMs: Long,
        nonce: String,
        witnessBase64: String
    ): Map<String, Any?> {
        require(witnessBase64.isNotBlank()) { "witnessBase64 is required" }
        val body = bodyWithBodyAuthFreshness(bodyFields, accountId, timestampMs, nonce)
        body[BODY_WITNESS_BASE64] = witnessBase64
        return body
    }

    /** Build canonical signing headers with generated freshness metadata. */
    @JvmStatic
    fun buildHeaders(
        method: String,
        uri: URI,
        body: ByteArray?,
        accountId: String,
        privateKey: PrivateKey
    ): Map<String, String> =
        buildHeaders(method, uri, body, accountId, privateKey, System.currentTimeMillis(), randomNonce())

    /** Build canonical signing headers with explicit freshness metadata. */
    @JvmStatic
    fun buildHeaders(
        method: String,
        uri: URI,
        body: ByteArray?,
        accountId: String,
        privateKey: PrivateKey,
        timestampMs: Long,
        nonce: String
    ): Map<String, String> {
        require(accountId.isNotBlank()) { "accountId is required" }
        require(nonce.isNotBlank()) { "nonce is required" }
        val message = canonicalRequestSignatureMessage(method, uri, body, timestampMs, nonce)
        val signatureBytes = signEd25519(privateKey, message)
        return mapOf(
            HEADER_ACCOUNT to accountId,
            HEADER_SIGNATURE to Base64.getEncoder().encodeToString(signatureBytes),
            HEADER_TIMESTAMP_MS to timestampMs.toString(),
            HEADER_NONCE to nonce,
        )
    }

    private fun bodyWithBodyAuthFreshness(
        bodyFields: Map<String, Any?>,
        accountId: String,
        timestampMs: Long,
        nonce: String
    ): LinkedHashMap<String, Any?> {
        require(accountId.isNotBlank()) { "accountId is required" }
        require(nonce.isNotBlank()) { "nonce is required" }
        val body = LinkedHashMap<String, Any?>(bodyFields)
        body[BODY_ACCOUNT_ID] = accountId
        body[BODY_TIMESTAMP_MS] = timestampMs
        body[BODY_NONCE] = nonce
        body.remove(BODY_SIGNATURE_BASE64)
        body.remove(BODY_WITNESS_BASE64)
        return body
    }

    private fun signEd25519(privateKey: PrivateKey, message: ByteArray): ByteArray {
        try {
            val signer = Signature.getInstance("Ed25519")
            signer.initSign(privateKey)
            signer.update(message)
            return signer.sign()
        } catch (ex: Exception) {
            throw IllegalStateException("failed to sign canonical request", ex)
        }
    }

    private fun urlEncode(value: String): String {
        try {
            return URLEncoder.encode(value, StandardCharsets.UTF_8.toString())
        } catch (ex: Exception) {
            throw IllegalStateException("failed to encode query component", ex)
        }
    }

    private fun urlDecode(value: String): String {
        try {
            return URLDecoder.decode(value, StandardCharsets.UTF_8.toString())
        } catch (ex: Exception) {
            throw IllegalArgumentException("failed to decode query component", ex)
        }
    }

    private fun randomNonce(): String {
        val bytes = ByteArray(16)
        NONCE_RANDOM.nextBytes(bytes)
        return hex(bytes)
    }

    private fun hex(bytes: ByteArray): String {
        val digits = "0123456789abcdef"
        val builder = StringBuilder(bytes.size * 2)
        for (b in bytes) {
            val value = b.toInt() and 0xff
            builder.append(digits[value ushr 4])
            builder.append(digits[value and 0x0f])
        }
        return builder.toString()
    }
}
