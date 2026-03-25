package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.Locale

/** Shared transport-safety checks for SDK requests that carry credentials or raw private keys. */
internal object TransportSecurity {
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
        headers?.keys?.any {
            when (it.lowercase(Locale.ROOT)) {
                "authorization", "x-api-token" -> true
                else -> false
            }
        } == true

    private fun isSensitive(headers: Map<String, String>?, body: ByteArray?): Boolean =
        headersContainCredentials(headers) || bodyContainsPrivateKey(body)

    private fun bodyContainsPrivateKey(body: ByteArray?): Boolean {
        if (body == null || body.isEmpty()) return false
        return String(body, StandardCharsets.UTF_8).lowercase(Locale.ROOT).contains("\"private_key\"")
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
