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
        val checkedMethod = requireHttpMethodToken(method)
        val path = requireCanonicalRawPath(uri)
        val query = canonicalQueryString(uri.rawQuery)
        val bodyBytes = body ?: ByteArray(0)
        val digest: ByteArray
        try {
            digest = MessageDigest.getInstance("SHA-256").digest(bodyBytes)
        } catch (ex: Exception) {
            throw IllegalStateException("sha256 unavailable", ex)
        }
        val rendered = "${checkedMethod.uppercase(Locale.ROOT)}\n$path\n$query\n${hex(digest)}"
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
     * [HEADER_ACCOUNT]; canonical ASCII aliases are emitted unchanged.
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
     * [HEADER_ACCOUNT]; canonical ASCII aliases are emitted unchanged.
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
        val checkedAccountId = requireCanonicalAuthAccount(accountId)
        requireExactNonBlank(nonce, "nonce")
        val accountHeader = canonicalAccountHeaderValue(checkedAccountId)
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
                .canonicalHex()
        } catch (_: AccountAddressException) {
            require(accountId.all { it.code in 0x21..0x7e }) {
                "accountId must be a canonical I105 account or canonical ASCII account alias"
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
        val checkedAccountId = requireCanonicalAuthAccount(accountId)
        requireExactNonBlank(nonce, "nonce")
        val body = LinkedHashMap<String, Any?>(bodyFields)
        body[BODY_ACCOUNT_ID] = checkedAccountId
        body[BODY_TIMESTAMP_MS] = timestampMs
        body[BODY_NONCE] = nonce
        body.remove(BODY_SIGNATURE_BASE64)
        body.remove(BODY_WITNESS_BASE64)
        return body
    }

    private fun requireCanonicalAuthAccount(accountId: String): String {
        requireExactNonBlank(accountId, "accountId")
        try {
            AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null)
            return accountId
        } catch (_: AccountAddressException) {
            require(isCanonicalAsciiAccountAlias(accountId)) {
                "accountId must be a canonical I105 account or canonical ASCII account alias"
            }
            return accountId
        }
    }

    // This is wire-safe structural admission only. Torii owns UTS-46 and alias resolution.
    private fun isCanonicalAsciiAccountAlias(value: String): Boolean {
        val separator = value.indexOf('@')
        if (
            value.startsWith("0x") ||
            separator <= 0 ||
            separator != value.lastIndexOf('@') ||
            separator == value.lastIndex ||
            value.any { it.code !in 0x21..0x7e }
        ) {
            return false
        }
        val scope = value.substring(separator + 1).split('.', limit = 3)
        return scope.size in 1..2 &&
            isCanonicalAsciiAliasSegment(value.substring(0, separator)) &&
            scope.all(::isCanonicalAsciiAliasSegment)
    }

    private fun isCanonicalAsciiAliasSegment(value: String): Boolean =
        value.length in 1..63 &&
            value.first() != '-' &&
            value.last() != '-' &&
            (
                value.length < 4 ||
                    value[2] != '-' ||
                    value[3] != '-' ||
                    value.startsWith("xn--")
            ) &&
            value.all { character ->
                character in 'a'..'z' ||
                    character in '0'..'9' ||
                    character == '-' ||
                    character == '_'
            }

    private fun requireHttpMethodToken(method: String): String {
        require(method.isNotEmpty()) { "canonical request method must not be empty" }
        require(
            method.length <= CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 &&
                method.toByteArray(StandardCharsets.UTF_8).size <= CANONICAL_REQUEST_MAX_METHOD_BYTES_V1
        ) {
            "canonical request method exceeds $CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 UTF-8 bytes"
        }
        require(method.all(::isHttpTokenCharacter)) {
            "canonical request method must be an ASCII HTTP token"
        }
        return method
    }

    private fun isHttpTokenCharacter(value: Char): Boolean =
        value in 'A'..'Z' ||
            value in 'a'..'z' ||
            value in '0'..'9' ||
            value == '!' ||
            value == '#' ||
            value == '$' ||
            value == '%' ||
            value == '&' ||
            value == '\'' ||
            value == '*' ||
            value == '+' ||
            value == '-' ||
            value == '.' ||
            value == '^' ||
            value == '_' ||
            value == '`' ||
            value == '|' ||
            value == '~'

    private fun requireCanonicalRawPath(uri: URI): String {
        require(!uri.isOpaque) { "canonical request URI must be hierarchical" }
        require(uri.rawFragment == null) { "canonical request URI must not contain a fragment" }

        val scheme = uri.scheme
        val authority = uri.rawAuthority
        if (scheme == null) {
            require(authority == null) { "canonical request URI must not be scheme-relative" }
        } else {
            require(
                authority != null &&
                    (scheme.equals("http", ignoreCase = true) || scheme.equals("https", ignoreCase = true))
            ) {
                "canonical request absolute URI must use HTTP(S) with an authority"
            }
        }

        val rawPath = uri.rawPath.orEmpty()
        val path = if (rawPath.isEmpty() && scheme != null) "/" else rawPath
        require(path.isNotEmpty() && path.startsWith('/') && !path.startsWith("//")) {
            "canonical request path must be an exact root-relative path"
        }
        require(path.all { it.code in 0x21..0x7e }) {
            "canonical request path must contain exact ASCII wire bytes"
        }
        require(path.length <= CANONICAL_REQUEST_MAX_PATH_BYTES_V1) {
            "canonical request path exceeds $CANONICAL_REQUEST_MAX_PATH_BYTES_V1 ASCII wire bytes"
        }
        require(hasSafeCanonicalPathSegments(path)) {
            "canonical request path must use well-formed escapes without dot segments"
        }
        return path
    }

    private fun hasSafeCanonicalPathSegments(path: String): Boolean {
        val structuralPath = StringBuilder(path.length)
        var index = 0
        while (index < path.length) {
            if (path[index] != '%') {
                structuralPath.append(path[index++])
                continue
            }
            if (index + 2 >= path.length) return false
            val high = hexValue(path[index + 1].code)
            val low = hexValue(path[index + 2].code)
            if (high < 0 || low < 0) return false
            when (val decoded = (high shl 4) or low) {
                '.'.code -> structuralPath.append(decoded.toChar())
                else -> structuralPath.append('\u0000')
            }
            index += 3
        }
        return structuralPath.toString().split('/').none { it == "." || it == ".." }
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
        return decodeUtf8LossyLikeRust(decoded.toByteArray())
    }

    /**
     * Decode UTF-8 with the same malformed-sequence boundaries as Rust's
     * `String::from_utf8_lossy`. The JVM decoder consumes an encoded surrogate
     * such as `ED A0 80` as one malformed unit, while Rust replaces each byte;
     * canonical request signatures must preserve the Rust/Torii grouping.
     */
    private fun decodeUtf8LossyLikeRust(bytes: ByteArray): String {
        val decoded = StringBuilder(bytes.size)
        var index = 0
        while (index < bytes.size) {
            val first = bytes[index].toInt() and 0xff
            when {
                first < 0x80 -> {
                    decoded.append(first.toChar())
                    index += 1
                }
                first in 0xc2..0xdf -> {
                    if (index + 1 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val second = bytes[index + 1].toInt() and 0xff
                    if (!isUtf8Continuation(second)) {
                        decoded.append('\uFFFD')
                        index += 1
                        continue
                    }
                    decoded.append(((first and 0x1f) shl 6 or (second and 0x3f)).toChar())
                    index += 2
                }
                first in 0xe0..0xef -> {
                    if (index + 1 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val second = bytes[index + 1].toInt() and 0xff
                    val validSecond = when (first) {
                        0xe0 -> second in 0xa0..0xbf
                        0xed -> second in 0x80..0x9f
                        else -> isUtf8Continuation(second)
                    }
                    if (!validSecond) {
                        decoded.append('\uFFFD')
                        index += 1
                        continue
                    }
                    if (index + 2 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val third = bytes[index + 2].toInt() and 0xff
                    if (!isUtf8Continuation(third)) {
                        decoded.append('\uFFFD')
                        index += 2
                        continue
                    }
                    val codePoint =
                        ((first and 0x0f) shl 12) or
                            ((second and 0x3f) shl 6) or
                            (third and 0x3f)
                    decoded.append(codePoint.toChar())
                    index += 3
                }
                first in 0xf0..0xf4 -> {
                    if (index + 1 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val second = bytes[index + 1].toInt() and 0xff
                    val validSecond = when (first) {
                        0xf0 -> second in 0x90..0xbf
                        0xf4 -> second in 0x80..0x8f
                        else -> isUtf8Continuation(second)
                    }
                    if (!validSecond) {
                        decoded.append('\uFFFD')
                        index += 1
                        continue
                    }
                    if (index + 2 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val third = bytes[index + 2].toInt() and 0xff
                    if (!isUtf8Continuation(third)) {
                        decoded.append('\uFFFD')
                        index += 2
                        continue
                    }
                    if (index + 3 >= bytes.size) {
                        decoded.append('\uFFFD')
                        index = bytes.size
                        continue
                    }
                    val fourth = bytes[index + 3].toInt() and 0xff
                    if (!isUtf8Continuation(fourth)) {
                        decoded.append('\uFFFD')
                        index += 3
                        continue
                    }
                    val codePoint =
                        ((first and 0x07) shl 18) or
                            ((second and 0x3f) shl 12) or
                            ((third and 0x3f) shl 6) or
                            (fourth and 0x3f)
                    decoded.append(Character.toChars(codePoint))
                    index += 4
                }
                else -> {
                    decoded.append('\uFFFD')
                    index += 1
                }
            }
        }
        return decoded.toString()
    }

    private fun isUtf8Continuation(value: Int): Boolean = value in 0x80..0xbf

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
