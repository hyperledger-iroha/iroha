package org.hyperledger.iroha.sdk.connect

import java.net.URI
import java.net.URISyntaxException
import java.net.URLDecoder
import java.util.Locale
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Parsed wallet-role request from an `iroha://connect?...` deep link. */
class ConnectWalletRequest private constructor(
    @JvmField val sidBase64Url: String,
    sessionId: ByteArray,
    @JvmField val token: String,
    @JvmField val chainId: String?,
    @JvmField val baseUri: URI,
    @JvmField val webSocketUri: URI,
) {
    private val _sessionId: ByteArray = sessionId.copyOf()

    fun sessionId(): ByteArray = _sessionId.clone()

    /** Stable short fingerprint used by UI/testing to correlate sessions without exposing full tokens. */
    fun sessionFingerprintHex(): String {
        val digest = Blake2b.digest(_sessionId, 8)
        val builder = StringBuilder(digest.size * 2)
        for (b in digest) {
            builder.append(String.format(Locale.ROOT, "%02x", b.toInt() and 0xFF))
        }
        return builder.toString()
    }

    companion object {
        private const val SCHEME = "iroha"
        private const val LAUNCH_SCHEME = "irohaconnect"
        private const val HOST = "connect"
        private const val LAUNCH_HOST = "wc"
        private const val SID_LENGTH = 32

        @JvmStatic
        @Throws(ConnectProtocolException::class)
        fun parse(uri: URI, defaultBaseUri: URI): ConnectWalletRequest {
            val normalizedUri = normalizeConnectUri(uri)
            val scheme = normalize(normalizedUri.scheme)
            val host = normalize(normalizedUri.host)
            if (scheme != SCHEME || host != HOST) {
                throw ConnectProtocolException(
                    "Connect deep link must use iroha://connect or irohaconnect://connect",
                )
            }

            val query = parseQuery(normalizedUri.rawQuery)
            val sid = firstPresent(query, "sid")
            if (sid.isNullOrBlank()) {
                throw ConnectProtocolException("Missing required query parameter: sid")
            }
            val sessionId = decodeBase64Url(sid)
            if (sessionId.size != SID_LENGTH) {
                throw ConnectProtocolException("Connect sid must decode to 32 bytes")
            }

            val token = firstNonBlank(
                firstPresent(query, "token_wallet"),
                firstPresent(query, "tokenWallet"),
                firstPresent(query, "token"),
            )
            if (token.isNullOrBlank()) {
                throw ConnectProtocolException("Missing required query parameter: token_wallet")
            }

            val chainId = trimToNull(firstPresent(query, "chain_id"))
            val base = resolveBaseUri(trimToNull(firstPresent(query, "node")), defaultBaseUri)
            val wsUri = buildWalletWebSocketUri(base, sid)

            return ConnectWalletRequest(sid, sessionId, token, chainId, base, wsUri)
        }

        @JvmStatic
        @Throws(ConnectProtocolException::class)
        fun parse(rawUri: String, defaultBaseUri: URI): ConnectWalletRequest {
            try {
                return parse(URI(rawUri), defaultBaseUri)
            } catch (ex: URISyntaxException) {
                throw ConnectProtocolException("Connect deep link URI is malformed", ex)
            }
        }

        private fun normalize(value: String?): String =
            value?.trim()?.lowercase(Locale.ROOT) ?: ""

        private fun trimToNull(value: String?): String? {
            val trimmed = value?.trim()
            return if (trimmed.isNullOrEmpty()) null else trimmed
        }

        private fun firstPresent(map: Map<String, String>, key: String): String? = map[key]

        private fun firstNonBlank(vararg values: String?): String? {
            for (value in values) {
                if (!value.isNullOrBlank()) return value.trim()
            }
            return null
        }

        @Throws(ConnectProtocolException::class)
        private fun normalizeConnectUri(uri: URI): URI {
            val scheme = normalize(uri.scheme)
            val host = normalize(uri.host)
            if (scheme == SCHEME && host == HOST) {
                return uri
            }
            if (scheme == LAUNCH_SCHEME && host == HOST) {
                return rebuildCanonicalConnectUri(uri)
            }
            if (scheme == LAUNCH_SCHEME && host == LAUNCH_HOST) {
                val query = parseQuery(uri.rawQuery)
                val embeddedUri = trimToNull(firstPresent(query, "uri"))
                    ?: throw ConnectProtocolException("Missing required query parameter: uri")
                try {
                    return normalizeConnectUri(URI(embeddedUri))
                } catch (ex: URISyntaxException) {
                    throw ConnectProtocolException("Embedded connect URI is malformed", ex)
                }
            }
            return uri
        }

        @Throws(ConnectProtocolException::class)
        private fun rebuildCanonicalConnectUri(uri: URI): URI = try {
            URI(
                SCHEME,
                uri.rawAuthority,
                uri.rawPath,
                uri.rawQuery,
                uri.rawFragment,
            )
        } catch (ex: URISyntaxException) {
            throw ConnectProtocolException("Connect deep link URI is malformed", ex)
        }

        private fun parseQuery(rawQuery: String?): Map<String, String> {
            val query = LinkedHashMap<String, String>()
            if (rawQuery.isNullOrEmpty()) return query
            for (part in rawQuery.split("&")) {
                if (part.isEmpty()) continue
                val idx = part.indexOf('=')
                val rawKey = if (idx >= 0) part.substring(0, idx) else part
                val rawValue = if (idx >= 0) part.substring(idx + 1) else ""
                val key = urlDecode(rawKey)
                if (!query.containsKey(key)) {
                    query[key] = urlDecode(rawValue)
                }
            }
            return query
        }

        private fun urlDecode(value: String): String =
            URLDecoder.decode(value, "UTF-8")

        @Throws(ConnectProtocolException::class)
        private fun decodeBase64Url(value: String): ByteArray {
            try {
                var normalized = value.replace('-', '+').replace('_', '/')
                val remainder = normalized.length % 4
                if (remainder != 0) {
                    normalized += "=".repeat(4 - remainder)
                }
                return java.util.Base64.getDecoder().decode(normalized)
            } catch (ex: IllegalArgumentException) {
                throw ConnectProtocolException("Connect sid is not valid base64url", ex)
            }
        }

        @Throws(ConnectProtocolException::class)
        private fun resolveBaseUri(nodeValue: String?, fallback: URI): URI {
            if (nodeValue.isNullOrEmpty()) return fallback
            var parsed = tryParse(nodeValue)
            if (parsed != null && parsed.scheme != null && parsed.host != null) {
                val normalizedScheme = normalize(parsed.scheme)
                if (normalizedScheme == "http" || normalizedScheme == "https") {
                    return parsed
                }
            }
            parsed = tryParse("https://$nodeValue")
            if (parsed != null && parsed.host != null) {
                return parsed
            }
            throw ConnectProtocolException("Invalid node parameter in connect link: $nodeValue")
        }

        private fun tryParse(raw: String): URI? = try {
            URI(raw)
        } catch (_: URISyntaxException) {
            null
        }

        @Throws(ConnectProtocolException::class)
        private fun buildWalletWebSocketUri(base: URI, sid: String): URI {
            val scheme = normalize(base.scheme)
            val wsScheme = when (scheme) {
                "https" -> "wss"
                "http" -> "ws"
                else -> throw ConnectProtocolException("Connect base URI must use http/https")
            }
            val host = base.host
            if (host.isNullOrBlank()) {
                throw ConnectProtocolException("Connect base URI is missing host")
            }
            val port = base.port
            val query = "sid=$sid&role=wallet"
            try {
                return URI(wsScheme, null, host, port, "/v1/connect/ws", query, null)
            } catch (ex: URISyntaxException) {
                throw ConnectProtocolException("Failed to build connect websocket URI", ex)
            }
        }
    }
}
