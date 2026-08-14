package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.Signature
import java.util.Base64
import java.util.Locale
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.core.model.NetworkId

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
    const val CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64
    const val CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024
    const val CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32
    const val CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024
    const val CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 = 36 * 1024
    private const val BODY_WITNESS_BASE64 = "witness_base64"

    private val NONCE_RANDOM = SecureRandom()
    private val NETWORK_DOMAIN = "iroha.app.request.network.v1\u0000".toByteArray(StandardCharsets.UTF_8)

    /** Canonicalise a raw query string by decoding, sorting, and re-encoding. */
    @JvmStatic
    fun canonicalQueryString(raw: String?): String {
        if (raw.isNullOrEmpty()) return ""
        require(raw.toByteArray(StandardCharsets.UTF_8).size <= CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1) {
            "canonical request query exceeds $CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 raw UTF-8 bytes"
        }
        val pairs = ArrayList<Pair<String, String>>()
        for (component in raw.split("&")) {
            if (component.isEmpty()) continue
            require(pairs.size < CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1) {
                "canonical request query exceeds $CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 pairs"
            }
            val kv = component.split("=", limit = 2)
            val key = if (kv.isNotEmpty()) kv[0] else ""
            val value = if (kv.size > 1) kv[1] else ""
            pairs.add(urlDecode(key) to urlDecode(value))
        }
        pairs.sortWith { left, right ->
            val keyOrder = compareUtf8(left.first, right.first)
            if (keyOrder != 0) keyOrder else compareUtf8(left.second, right.second)
        }
        return pairs.joinToString("&") { "${urlEncode(it.first)}=${urlEncode(it.second)}" }
    }

    /** Build canonical request bytes for signing. */
    @JvmStatic
    fun canonicalRequestMessage(method: String, uri: URI, body: ByteArray?): ByteArray {
        require(
            method.length <= CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 &&
                method.toByteArray(StandardCharsets.UTF_8).size <= CANONICAL_REQUEST_MAX_METHOD_BYTES_V1
        ) {
            "canonical request method exceeds $CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 UTF-8 bytes"
        }
        val path = uri.rawPath?.takeIf { it.isNotEmpty() } ?: "/"
        require(
            path.length <= CANONICAL_REQUEST_MAX_PATH_BYTES_V1 &&
                path.toByteArray(StandardCharsets.UTF_8).size <= CANONICAL_REQUEST_MAX_PATH_BYTES_V1
        ) {
            "canonical request path exceeds $CANONICAL_REQUEST_MAX_PATH_BYTES_V1 UTF-8 bytes"
        }
        val query = canonicalQueryString(uri.rawQuery)
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

    /** Build canonical request bytes bound to an exact network and freshness metadata. */
    @JvmStatic
    fun canonicalRequestSignatureMessage(
        networkId: NetworkId,
        method: String,
        uri: URI,
        body: ByteArray?,
        timestampMs: Long,
        nonce: String
    ): ByteArray {
        require(timestampMs >= 0) { "timestampMs must be non-negative" }
        requireExactNonBlank(nonce, "nonce")
        val base = canonicalRequestMessage(method, uri, body)
        val suffix = "\n$timestampMs\n$nonce".toByteArray(StandardCharsets.UTF_8)
        return NETWORK_DOMAIN + networkId.bytes() + base + suffix
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
        networkId: NetworkId,
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        timestampMs: Long,
        nonce: String
    ): ByteArray = canonicalRequestSignatureMessage(
        networkId,
        method,
        uri,
        unsignedBodyAuthJson(bodyFields),
        timestampMs,
        nonce,
    )

    /** Build the top-level fields required for single-signature body auth. */
    @JvmStatic
    fun buildBodySignatureFields(
        networkId: NetworkId,
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        accountId: String,
        privateKey: PrivateKey
    ): Map<String, Any?> =
        buildBodySignatureFields(networkId, method, uri, bodyFields, accountId, privateKey, System.currentTimeMillis(), randomNonce())

    /** Build the top-level fields required for single-signature body auth with explicit freshness metadata. */
    @JvmStatic
    fun buildBodySignatureFields(
        networkId: NetworkId,
        method: String,
        uri: URI,
        bodyFields: Map<String, Any?>,
        accountId: String,
        privateKey: PrivateKey,
        timestampMs: Long,
        nonce: String
    ): Map<String, Any?> {
        val unsigned = bodyWithBodyAuthFreshness(bodyFields, accountId, timestampMs, nonce)
        val message = canonicalBodyAuthSignatureMessage(networkId, method, uri, unsigned, timestampMs, nonce)
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
        networkId: NetworkId,
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
        body.putAll(buildBodySignatureFields(networkId, method, uri, body, accountId, privateKey, timestampMs, nonce))
        return body
    }

    /**
     * Build canonical signing headers with generated freshness metadata.
     *
     * Canonical I105 identities are emitted as lowercase canonical hex in
     * [HEADER_ACCOUNT]; printable ASCII aliases are emitted unchanged.
     */
    @JvmStatic
    fun buildHeaders(
        networkId: NetworkId,
        method: String,
        uri: URI,
        body: ByteArray?,
        accountId: String,
        privateKey: PrivateKey
    ): Map<String, String> =
        buildHeaders(networkId, method, uri, body, accountId, privateKey, System.currentTimeMillis(), randomNonce())

    /**
     * Build canonical signing headers with explicit freshness metadata.
     *
     * Canonical I105 identities are emitted as lowercase canonical hex in
     * [HEADER_ACCOUNT]; printable ASCII aliases are emitted unchanged.
     */
    @JvmStatic
    fun buildHeaders(
        networkId: NetworkId,
        method: String,
        uri: URI,
        body: ByteArray?,
        accountId: String,
        privateKey: PrivateKey,
        timestampMs: Long,
        nonce: String
    ): Map<String, String> {
        requireExactNonBlank(accountId, "accountId")
        requireExactNonBlank(nonce, "nonce")
        val accountHeader = canonicalAccountHeaderValue(accountId)
        val message = canonicalRequestSignatureMessage(networkId, method, uri, body, timestampMs, nonce)
        val signatureBytes = signEd25519(privateKey, message)
        return mapOf(
            HEADER_ACCOUNT to accountHeader,
            HEADER_SIGNATURE to Base64.getEncoder().encodeToString(signatureBytes),
            HEADER_TIMESTAMP_MS to timestampMs.toString(),
            HEADER_NONCE to nonce,
        )
    }

    private fun canonicalAccountHeaderValue(accountId: String): String {
        try {
            return AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null)
                .address
                .canonicalHex()
        } catch (_: AccountAddressException) {
            require(accountId.all { it.code in 0x21..0x7e }) {
                "accountId must be a canonical I105 account or printable ASCII account alias"
            }
            return accountId
        }
    }

    private fun bodyWithBodyAuthFreshness(
        bodyFields: Map<String, Any?>,
        accountId: String,
        timestampMs: Long,
        nonce: String
    ): LinkedHashMap<String, Any?> {
        requireExactNonBlank(accountId, "accountId")
        requireExactNonBlank(nonce, "nonce")
        val body = LinkedHashMap<String, Any?>(bodyFields)
        body[BODY_ACCOUNT_ID] = accountId
        body[BODY_TIMESTAMP_MS] = timestampMs
        body[BODY_NONCE] = nonce
        body.remove(BODY_SIGNATURE_BASE64)
        body.remove(BODY_WITNESS_BASE64)
        return body
    }

    private fun requireExactNonBlank(value: String, field: String) {
        require(value.isNotEmpty() && value.any { !it.isWhitespace() }) { "$field is required" }
        require(!value.first().isWhitespace() && !value.last().isWhitespace()) {
            "$field must not contain surrounding whitespace"
        }
        if (field == "accountId") {
            require(value.toByteArray(StandardCharsets.UTF_8).size <= CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1) {
                "accountId exceeds $CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 UTF-8 bytes"
            }
        }
        if (field == "nonce") {
            require(value.toByteArray(StandardCharsets.UTF_8).size <= 256 && value.all { it.code in 0x21..0x7e }) {
                "nonce must contain 1...256 non-whitespace ASCII bytes"
            }
        }
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
        val raw = value.toByteArray(StandardCharsets.UTF_8)
        val decoded = ByteArrayOutputStream(raw.size)
        var index = 0
        while (index < raw.size) {
            val byte = raw[index].toInt() and 0xff
            if (byte == '+'.code) {
                decoded.write(' '.code)
                index += 1
            } else if (byte == '%'.code && index + 2 < raw.size) {
                val high = hexValue(raw[index + 1].toInt() and 0xff)
                val low = hexValue(raw[index + 2].toInt() and 0xff)
                if (high >= 0 && low >= 0) {
                    decoded.write((high shl 4) or low)
                    index += 3
                } else {
                    decoded.write(byte)
                    index += 1
                }
            } else {
                decoded.write(byte)
                index += 1
            }
        }
        return String(decoded.toByteArray(), StandardCharsets.UTF_8)
    }

    private fun hexValue(value: Int): Int = when (value) {
        in '0'.code..'9'.code -> value - '0'.code
        in 'A'.code..'F'.code -> value - 'A'.code + 10
        in 'a'.code..'f'.code -> value - 'a'.code + 10
        else -> -1
    }

    private fun compareUtf8(left: String, right: String): Int {
        val leftBytes = left.toByteArray(StandardCharsets.UTF_8)
        val rightBytes = right.toByteArray(StandardCharsets.UTF_8)
        val sharedLength = minOf(leftBytes.size, rightBytes.size)
        for (index in 0 until sharedLength) {
            val difference =
                (leftBytes[index].toInt() and 0xff) - (rightBytes[index].toInt() and 0xff)
            if (difference != 0) return difference
        }
        return leftBytes.size - rightBytes.size
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
