package org.hyperledger.iroha.sdk.connect

import java.net.URI
import java.net.URISyntaxException
import java.net.URLDecoder
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Parsed wallet-role request from an `iroha://connect?...` deep link. */
class ConnectWalletRequest private constructor(
    @JvmField val sidBase64Url: String,
    sessionId: ByteArray,
    @JvmField val token: String,
    @JvmField val relayToken: String,
    @JvmField val networkId: NetworkId,
    appPublicKey: ByteArray,
    nonce: ByteArray,
    @JvmField val baseUri: URI,
    @JvmField val webSocketUri: URI,
) {
    private val _sessionId: ByteArray = sessionId.copyOf()
    private val _appPublicKey: ByteArray = appPublicKey.copyOf()
    private val _nonce: ByteArray = nonce.copyOf()
    private val openAccepted = AtomicBoolean(false)

    fun sessionId(): ByteArray = _sessionId.clone()
    fun appPublicKey(): ByteArray = _appPublicKey.clone()
    fun nonce(): ByteArray = _nonce.clone()

    /** Validates and consumes the one permitted application `Open` frame for this request. */
    @Throws(ConnectProtocolException::class)
    fun acceptOpen(rawFrame: ByteArray): OpenControl {
        val frame = ConnectFrameCodec.decode(rawFrame)
        if (!frame.sessionId().contentEquals(_sessionId)) {
            throw ConnectProtocolException("Connect Open sid does not match the launch request")
        }
        if (frame.direction != ConnectDirection.APP_TO_WALLET || frame.sequence != 1L) {
            throw ConnectProtocolException("Connect Open must be app-to-wallet sequence 1")
        }
        val open = frame.open
            ?: throw ConnectProtocolException("Expected a Connect Open control frame")
        if (!open.appPublicKey().contentEquals(_appPublicKey)) {
            throw ConnectProtocolException("Connect Open app_pk does not match the launch request")
        }
        if (open.networkId != networkId) {
            throw ConnectProtocolException("Connect Open network_id does not match the launch request")
        }
        if (!openAccepted.compareAndSet(false, true)) {
            throw ConnectProtocolException("Connect Open was already accepted")
        }
        return open
    }

    /** Builds the exact approval preimage after the launch-bound `Open` has been consumed. */
    @Throws(ConnectProtocolException::class)
    fun buildApprovePreimage(
        walletPublicKey: ByteArray,
        accountId: String,
        permissionsHash: ByteArray?,
        proofHash: ByteArray?,
    ): ByteArray {
        if (!openAccepted.get()) {
            throw ConnectProtocolException("Connect Open must be accepted before approval")
        }
        return ConnectCrypto.buildApprovePreimage(
            networkId,
            _sessionId,
            _appPublicKey,
            walletPublicKey,
            accountId,
            permissionsHash,
            proofHash,
            ConnectCrypto.relayAuthHash(_sessionId, relayToken),
        )
    }

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
        private const val HOST = "connect"
        private const val SID_LENGTH = 32

        @JvmStatic
        @Throws(ConnectProtocolException::class)
        fun parse(uri: URI, defaultBaseUri: URI): ConnectWalletRequest {
            if (uri.scheme != SCHEME || uri.host != HOST ||
                !uri.rawPath.isNullOrEmpty() || uri.rawFragment != null || uri.rawUserInfo != null
            ) {
                throw ConnectProtocolException("Connect deep link must use canonical iroha://connect")
            }

            val query = parseQuery(uri.rawQuery)
            val sid = query["sid"]
            if (sid.isNullOrEmpty()) {
                throw ConnectProtocolException("Missing required query parameter: sid")
            }
            val sessionId = decodeBase64Url(sid, "sid")
            if (sessionId.size != SID_LENGTH) {
                throw ConnectProtocolException("Connect sid must decode to 32 bytes")
            }

            for (retired in listOf("chain_id", "token_wallet", "tokenWallet", "token_relay", "tokenRelay")) {
                if (query.containsKey(retired)) {
                    throw ConnectProtocolException("Retired Connect query parameter: $retired")
                }
            }
            val token = query["token"]
            if (token.isNullOrBlank() || token.trim() != token) {
                throw ConnectProtocolException("Missing or invalid required query parameter: token")
            }
            val relayToken = query["relay"]
            if (relayToken.isNullOrBlank() || relayToken.trim() != relayToken) {
                throw ConnectProtocolException("Missing required query parameter: relay")
            }
            if (query["v"] != "1") {
                throw ConnectProtocolException("Connect v must be exactly 1")
            }
            if (query["role"] != "wallet") {
                throw ConnectProtocolException("Connect role must be exactly wallet")
            }
            val networkIdLiteral = query["network_id"]
                ?: throw ConnectProtocolException("Missing required query parameter: network_id")
            val networkId = try {
                NetworkId.parse(networkIdLiteral)
            } catch (ex: IllegalArgumentException) {
                throw ConnectProtocolException("Connect network_id is not canonical", ex)
            }
            val appPublicKey = decodeBase64Url(
                query["app_pk"]
                    ?: throw ConnectProtocolException("Missing required query parameter: app_pk"),
                "app_pk",
            )
            val nonce = decodeBase64Url(
                query["nonce"]
                    ?: throw ConnectProtocolException("Missing required query parameter: nonce"),
                "nonce",
            )
            if (appPublicKey.size != 32 || nonce.size != 16) {
                throw ConnectProtocolException("Connect app_pk and nonce must decode to 32 and 16 bytes")
            }
            val expectedSid = ConnectCrypto.deriveSessionId(networkId, appPublicKey, nonce)
            if (!sessionId.contentEquals(expectedSid)) {
                throw ConnectProtocolException("Connect sid does not match network_id, app_pk, and nonce")
            }
            val base = resolveBaseUri(query["node"], defaultBaseUri)
            val wsUri = buildWalletWebSocketUri(base, sid)

            return ConnectWalletRequest(
                sid,
                sessionId,
                token,
                relayToken,
                networkId,
                appPublicKey,
                nonce,
                base,
                wsUri,
            )
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

        @Throws(ConnectProtocolException::class)
        private fun parseQuery(rawQuery: String?): Map<String, String> {
            val query = LinkedHashMap<String, String>()
            if (rawQuery.isNullOrEmpty()) return query
            for (part in rawQuery.split("&")) {
                if (part.isEmpty()) continue
                val idx = part.indexOf('=')
                val rawKey = if (idx >= 0) part.substring(0, idx) else part
                val rawValue = if (idx >= 0) part.substring(idx + 1) else ""
                val key: String
                val value: String
                try {
                    key = urlDecode(rawKey)
                    value = urlDecode(rawValue)
                } catch (ex: IllegalArgumentException) {
                    throw ConnectProtocolException("Connect query contains invalid percent encoding", ex)
                }
                if (query.put(key, value) != null) {
                    throw ConnectProtocolException("Duplicate Connect query parameter: $key")
                }
            }
            return query
        }

        private fun urlDecode(value: String): String =
            URLDecoder.decode(value, "UTF-8")

        @Throws(ConnectProtocolException::class)
        private fun decodeBase64Url(value: String, field: String): ByteArray {
            try {
                var normalized = value.replace('-', '+').replace('_', '/')
                val remainder = normalized.length % 4
                if (remainder != 0) {
                    normalized += "=".repeat(4 - remainder)
                }
                val decoded = java.util.Base64.getDecoder().decode(normalized)
                val canonical = java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(decoded)
                if (canonical != value) {
                    throw ConnectProtocolException("Connect $field must use canonical base64url without padding")
                }
                return decoded
            } catch (ex: IllegalArgumentException) {
                throw ConnectProtocolException("Connect $field is not valid base64url", ex)
            }
        }

        @Throws(ConnectProtocolException::class)
        private fun resolveBaseUri(nodeValue: String?, defaultUri: URI): URI {
            if (nodeValue.isNullOrEmpty()) return defaultUri
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
