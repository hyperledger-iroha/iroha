package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.Locale

/** Shared transport-safety checks for SDK requests that carry credentials or raw private keys. */
internal object TransportSecurity {
    private val credentialHeaders = setOf(
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
    )

    private val sensitiveBodyFields = listOf(
        "\"private_key\"",
        "\"seed_hex\"",
        "\"seed_b64\"",
        "\"seed_base64\"",
        "\"key_seed_hex\"",
        "\"key_seed_b64\"",
    )

    fun requireHttpRequestAllowed(
        context: String,
        baseUri: URI,
        targetUri: URI,
        headers: Map<String, String>?,
        body: ByteArray?,
    ) {
        if (!isSensitive(headers, body)) return
        val targetScheme = normalize(targetUri.scheme)
        require(targetScheme == "https") {
            "$context refuses insecure transport over ${renderScheme(targetScheme)}; use https."
        }
        val baseScheme = normalize(baseUri.scheme)
        require(targetScheme == baseScheme) {
            "$context refuses sensitive requests over mismatched scheme ${renderScheme(targetScheme)}; use relative paths derived from the configured base URL."
        }
        require(sameAuthority(baseUri, targetUri, "https")) {
            "$context refuses sensitive requests to mismatched host ${renderHost(targetUri)}; use relative paths on the configured base URL."
        }
    }

    fun requireWebSocketRequestAllowed(
        context: String,
        baseUri: URI,
        targetUri: URI,
        headers: Map<String, String>?,
    ) {
        if (!headersContainCredentials(headers)) return
        val expectedScheme = expectedWebSocketScheme(baseUri)
        val targetScheme = normalize(targetUri.scheme)
        require(targetScheme == expectedScheme) {
            "$context refuses credentialed WebSocket requests over mismatched scheme ${renderScheme(targetScheme)}; use $expectedScheme URLs derived from the configured base URL."
        }
        require(sameAuthority(baseUri, targetUri, expectedScheme)) {
            "$context refuses credentialed WebSocket requests to mismatched host ${renderHost(targetUri)}; use the configured base URL host."
        }
        require(targetScheme == "wss") {
            "$context refuses insecure WebSocket protocol ${renderScheme(targetScheme)}; use wss."
        }
    }

    fun headersContainCredentials(headers: Map<String, String>?): Boolean =
        headers?.keys?.any { credentialHeaders.contains(it.lowercase(Locale.ROOT)) } == true

    private fun isSensitive(headers: Map<String, String>?, body: ByteArray?): Boolean =
        headersContainCredentials(headers) || bodyContainsSensitiveMaterial(body)

    private fun bodyContainsSensitiveMaterial(body: ByteArray?): Boolean {
        if (body == null || body.isEmpty()) return false
        val rendered = String(body, StandardCharsets.UTF_8).lowercase(Locale.ROOT)
        return sensitiveBodyFields.any(rendered::contains)
    }

    private fun expectedWebSocketScheme(baseUri: URI): String =
        when (normalize(baseUri.scheme)) {
            "https", "wss" -> "wss"
            else -> "ws"
        }

    private fun sameAuthority(lhs: URI, rhs: URI, defaultPortScheme: String): Boolean =
        normalize(lhs.host) == normalize(rhs.host) &&
            effectivePort(lhs, defaultPortScheme) == effectivePort(rhs, defaultPortScheme)

    private fun effectivePort(uri: URI, defaultPortScheme: String): Int {
        if (uri.port != -1) return uri.port
        return when (normalize(uri.scheme).ifEmpty { defaultPortScheme }) {
            "http", "ws" -> 80
            "https", "wss" -> 443
            else -> -1
        }
    }

    private fun renderHost(uri: URI): String = uri.host ?: uri.toString()

    private fun renderScheme(scheme: String): String = if (scheme.isEmpty()) "unknown" else scheme

    private fun normalize(value: String?): String = value?.lowercase(Locale.ROOT) ?: ""
}
