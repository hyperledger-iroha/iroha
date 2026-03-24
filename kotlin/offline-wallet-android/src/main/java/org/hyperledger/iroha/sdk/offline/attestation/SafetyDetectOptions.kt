package org.hyperledger.iroha.sdk.offline.attestation

import java.net.URI
import java.time.Duration

/** Configuration controlling the optional Huawei Safety Detect attestation flow. */
class SafetyDetectOptions private constructor(
    val enabled: Boolean,
    val oauthEndpoint: URI,
    val attestationEndpoint: URI,
    val clientId: String?,
    val clientSecret: String?,
    val packageName: String?,
    val signingDigestSha256: String?,
    val requestTimeout: Duration,
    val tokenSkew: Duration,
) {
    class Builder {
        private var enabled = true
        private var oauthEndpoint: URI = DEFAULT_OAUTH_ENDPOINT
        private var attestationEndpoint: URI = DEFAULT_APP_CHECK_ENDPOINT
        private var clientId: String? = null
        private var clientSecret: String? = null
        private var packageName: String? = null
        private var signingDigestSha256: String? = null
        private var requestTimeout: Duration = DEFAULT_TIMEOUT
        private var tokenSkew: Duration = DEFAULT_TOKEN_SKEW

        fun setEnabled(enabled: Boolean) = apply { this.enabled = enabled }
        fun setOauthEndpoint(uri: URI) = apply { oauthEndpoint = uri }
        fun setAttestationEndpoint(uri: URI) = apply { attestationEndpoint = uri }
        fun setClientId(clientId: String) = apply { this.clientId = clientId }
        fun setClientSecret(clientSecret: String) = apply { this.clientSecret = clientSecret }
        fun setPackageName(packageName: String) = apply { this.packageName = packageName }
        fun setSigningDigestSha256(digest: String) = apply { signingDigestSha256 = digest }
        fun setRequestTimeout(timeout: Duration) = apply { requestTimeout = timeout }
        fun setTokenSkew(skew: Duration) = apply { tokenSkew = skew }

        fun build(): SafetyDetectOptions {
            if (!enabled) return SafetyDetectOptions(
                enabled, oauthEndpoint, attestationEndpoint, clientId, clientSecret,
                packageName, signingDigestSha256, requestTimeout, tokenSkew,
            )
            require(!clientId.isNullOrBlank()) { "clientId must be set when Safety Detect is enabled" }
            require(!clientSecret.isNullOrBlank()) { "clientSecret must be set when Safety Detect is enabled" }
            require(!packageName.isNullOrBlank()) { "packageName must be set when Safety Detect is enabled" }
            require(!signingDigestSha256.isNullOrBlank()) { "signingDigestSha256 must be set when Safety Detect is enabled" }
            return SafetyDetectOptions(
                enabled, oauthEndpoint, attestationEndpoint, clientId, clientSecret,
                packageName, signingDigestSha256, requestTimeout, tokenSkew,
            )
        }
    }

    companion object {
        private val DEFAULT_TIMEOUT = Duration.ofSeconds(10)
        private val DEFAULT_TOKEN_SKEW = Duration.ofSeconds(30)
        private val DEFAULT_OAUTH_ENDPOINT = URI.create("https://oauth-login.cloud.huawei.com/oauth2/v3/token")
        private val DEFAULT_APP_CHECK_ENDPOINT = URI.create("https://safetydetectapi.hwclouds.com/v5/appcheck")

        @JvmStatic
        fun builder() = Builder()
    }
}
