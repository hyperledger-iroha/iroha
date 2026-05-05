package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.Signature
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/** Builds canonical request signatures for Torii app endpoints. */
@OptIn(ExperimentalEncodingApi::class)
object CanonicalRequestSigner {

    const val HEADER_ACCOUNT = "X-Iroha-Account"
    const val HEADER_SIGNATURE = "X-Iroha-Signature"

    /** Canonicalise a raw query string by decoding, sorting, and re-encoding. */
    @JvmStatic
    fun canonicalQueryString(raw: String?): String {
        if (raw.isNullOrEmpty()) return ""
        val pairs = ArrayList<Pair<String, String>>()
        for (component in raw.split("&", limit = -1)) {
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
        val rendered = "${method.uppercase()}\n$path\n$query\n${hex(digest)}"
        return rendered.toByteArray(StandardCharsets.UTF_8)
    }

    /** Build canonical signing headers (`X-Iroha-Account`/`X-Iroha-Signature`). */
    @JvmStatic
    fun buildHeaders(
        method: String,
        uri: URI,
        body: ByteArray?,
        accountId: String,
        privateKey: PrivateKey
    ): Map<String, String> {
        require(accountId.isNotBlank()) { "accountId is required" }
        val message = canonicalRequestMessage(method, uri, body)
        val signatureBytes: ByteArray
        try {
            val signer = Signature.getInstance("Ed25519")
            signer.initSign(privateKey)
            signer.update(message)
            signatureBytes = signer.sign()
        } catch (ex: Exception) {
            throw IllegalStateException("failed to sign canonical request", ex)
        }
        return mapOf(
            HEADER_ACCOUNT to accountId,
            HEADER_SIGNATURE to Base64.encode(signatureBytes)
        )
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

    private fun hex(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (b in bytes) {
            builder.append(String.format("%02x", b))
        }
        return builder.toString()
    }
}
