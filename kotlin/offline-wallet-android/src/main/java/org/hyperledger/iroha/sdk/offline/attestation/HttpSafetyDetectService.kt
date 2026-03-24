package org.hyperledger.iroha.sdk.offline.attestation

import java.net.URLEncoder
import java.time.Clock
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.atomic.AtomicReference
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/**
 * Default [SafetyDetectService] backed by HTTP endpoints provided by Huawei. Tokens are cached
 * locally so the SDK does not fetch OAuth credentials for every attestation attempt.
 */
class HttpSafetyDetectService : SafetyDetectService {

    private val executor: HttpTransportExecutor
    private val options: SafetyDetectOptions
    private val clock: Clock
    private val cachedToken = AtomicReference<AccessToken>()

    constructor(executor: HttpTransportExecutor, options: SafetyDetectOptions) : this(executor, options, Clock.systemUTC())

    internal constructor(executor: HttpTransportExecutor, options: SafetyDetectOptions, clock: Clock) {
        this.executor = executor
        this.options = options
        this.clock = clock
        require(options.enabled) { "SafetyDetectOptions must be enabled" }
    }

    override fun fetch(request: SafetyDetectRequest): CompletableFuture<SafetyDetectAttestation> =
        resolveAccessToken().thenCompose { token -> executeAttestation(request, token) }

    private fun executeAttestation(
        request: SafetyDetectRequest, accessToken: String,
    ): CompletableFuture<SafetyDetectAttestation> {
        val payload = LinkedHashMap<String, Any>()
        payload["app_id"] = request.appId
        payload["nonce"] = encodeNonce(request.nonceHex)
        payload["package_name"] = request.packageName
        payload["sign_cert_sha256"] = request.signingDigestSha256
        payload["certificate_id_hex"] = request.certificateIdHex
        val evaluations = request.requiredEvaluations()
        if (evaluations.isNotEmpty()) payload["required_evaluations"] = evaluations
        val maxAge = request.maxTokenAgeMs()
        if (maxAge != null && maxAge > 0) payload["max_token_age_ms"] = maxAge
        val body = JsonEncoder.encode(payload)
        val httpRequest = TransportRequest.builder()
            .setUri(options.attestationEndpoint)
            .setMethod("POST")
            .setTimeout(options.requestTimeout)
            .addHeader("Content-Type", "application/json")
            .addHeader("Authorization", "Bearer $accessToken")
            .setBody(body.toByteArray(Charsets.UTF_8))
            .build()
        val fetchedAt = clock.millis()
        return executor.execute(httpRequest).thenApply { response -> parseAttestation(response, fetchedAt) }
    }

    private fun parseAttestation(response: TransportResponse, fetchedAt: Long): SafetyDetectAttestation {
        if (response.statusCode != 200) {
            throw SafetyDetectException(
                "Safety Detect attestation failed with status ${response.statusCode}: ${response.message}")
        }
        val parsed = JsonParser.parse(String(response.body, Charsets.UTF_8))
        check(parsed is Map<*, *>) { "Safety Detect attestation response is not a JSON object" }
        val tokenValue = parsed["token"]
        if (tokenValue !is String || tokenValue.isBlank()) {
            throw SafetyDetectException("Safety Detect attestation response missing token")
        }
        return SafetyDetectAttestation(tokenValue, fetchedAt)
    }

    private fun resolveAccessToken(): CompletableFuture<String> {
        val cached = cachedToken.get()
        val now = clock.millis()
        if (cached != null && cached.isValid(now)) return CompletableFuture.completedFuture(cached.value)
        val request = TransportRequest.builder()
            .setUri(options.oauthEndpoint)
            .setMethod("POST")
            .setTimeout(options.requestTimeout)
            .addHeader("Content-Type", "application/x-www-form-urlencoded")
            .setBody(buildOauthBody().toByteArray(Charsets.UTF_8))
            .build()
        return executor.execute(request).thenApply { response -> parseAccessToken(response, now) }
    }

    private fun buildOauthBody(): String {
        val sb = StringBuilder()
        sb.append("grant_type=client_credentials")
        sb.append("&client_id=").append(urlEncode(options.clientId!!))
        sb.append("&client_secret=").append(urlEncode(options.clientSecret!!))
        return sb.toString()
    }

    private fun parseAccessToken(response: TransportResponse, requestedAtMs: Long): String {
        if (response.statusCode != 200) {
            throw SafetyDetectException(
                "Safety Detect OAuth failed with status ${response.statusCode}: ${response.message}")
        }
        val parsed = JsonParser.parse(String(response.body, Charsets.UTF_8))
        if (parsed !is Map<*, *>) throw SafetyDetectException("Safety Detect OAuth response is not a JSON object")
        val tokenValue = parsed["access_token"]
        if (tokenValue !is String || tokenValue.isBlank()) {
            throw SafetyDetectException("Safety Detect OAuth response missing access_token")
        }
        val expirySeconds = asLong(parsed["expires_in"], "expires_in")
        val expiresAt = requestedAtMs + Duration.ofSeconds(expirySeconds).toMillis() - options.tokenSkew.toMillis()
        val newToken = AccessToken(tokenValue, maxOf(expiresAt, requestedAtMs))
        cachedToken.set(newToken)
        return newToken.value
    }

    private class AccessToken(val value: String, val expiresAtMs: Long) {
        fun isValid(nowMs: Long): Boolean = nowMs < expiresAtMs
    }

    companion object {
        @JvmStatic
        fun createDefault(options: SafetyDetectOptions): HttpSafetyDetectService =
            HttpSafetyDetectService(PlatformHttpTransportExecutor.createDefault(), options)

        private fun asLong(value: Any?, field: String): Long {
            if (value is Number) {
                if (value is Float || value is Double) {
                    throw SafetyDetectException("Safety Detect OAuth field $field must be an integer")
                }
                return value.toLong()
            }
            if (value == null) throw SafetyDetectException("Safety Detect OAuth missing $field")
            return try { value.toString().toLong() }
            catch (ex: NumberFormatException) {
                throw SafetyDetectException("Safety Detect OAuth field $field is not numeric", ex)
            }
        }

        private fun encodeNonce(hex: String): String {
            val decoded = decodeHex(hex)
            return Base64.getEncoder().encodeToString(decoded)
        }

        private fun decodeHex(hex: String?): ByteArray {
            if (hex.isNullOrBlank()) return byteArrayOf()
            val normalized = hex.trim()
            if (normalized.length % 2 != 0) throw SafetyDetectException("Nonce hex must have an even length")
            val out = ByteArray(normalized.length / 2)
            for (i in normalized.indices step 2) {
                out[i / 2] = normalized.substring(i, i + 2).toInt(16).toByte()
            }
            return out
        }

        private fun urlEncode(value: String): String =
            URLEncoder.encode(value, Charsets.UTF_8)
    }
}
