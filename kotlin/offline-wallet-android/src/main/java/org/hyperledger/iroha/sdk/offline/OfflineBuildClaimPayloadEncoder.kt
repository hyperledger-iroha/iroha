package org.hyperledger.iroha.sdk.offline

/**
 * Encodes OfflineBuildClaim payload to Norito bytes for signature verification/signing checks.
 *
 * This encoder serializes the build-claim payload structure using the Norito codec.
 */
class OfflineBuildClaimPayloadEncoder private constructor() {
    companion object {
        private val NATIVE_AVAILABLE: Boolean

        init {
            var available: Boolean
            try {
                System.loadLibrary("connect_norito_bridge")
                available = true
            } catch (_: UnsatisfiedLinkError) {
                available = false
            }
            NATIVE_AVAILABLE = available
        }

        @JvmStatic
        fun isNativeAvailable(): Boolean = NATIVE_AVAILABLE

        /**
         * Encode OfflineBuildClaim payload to Norito bytes.
         *
         * @param claimIdHex 32-byte claim identifier as hex (64 chars)
         * @param platform build platform token (`Apple`/`Android`, with `ios` accepted as alias)
         * @param appId application identifier
         * @param buildNumber certified build number
         * @param issuedAtMs issuance timestamp in milliseconds
         * @param expiresAtMs expiration timestamp in milliseconds
         * @param lineageScope claim lineage scope (null/blank coerced to empty)
         * @param nonceHex 32-byte nonce as hex (64 chars)
         * @return Norito-encoded bytes
         * @throws IllegalStateException if native library is unavailable
         * @throws IllegalArgumentException if any parameter is invalid
         */
        @JvmStatic
        fun encode(
            claimIdHex: String,
            platform: String,
            appId: String,
            buildNumber: Long,
            issuedAtMs: Long,
            expiresAtMs: Long,
            lineageScope: String?,
            nonceHex: String,
        ): ByteArray {
            check(NATIVE_AVAILABLE) { "connect_norito_bridge is not available in this runtime" }
            val normalizedClaimIdHex = OfflineHashLiteral.parseHex(claimIdHex, "claimIdHex")
            val normalizedPlatform = normalizePlatform(platform)
            val normalizedAppId = requireNonBlank(appId, "appId")
            requireNonNegative(buildNumber, "buildNumber")
            requireNonNegative(issuedAtMs, "issuedAtMs")
            requireNonNegative(expiresAtMs, "expiresAtMs")
            val normalizedLineageScope =
                if (lineageScope.isNullOrBlank()) "" else lineageScope.trim()
            val normalizedNonceHex = OfflineHashLiteral.parseHex(nonceHex, "nonceHex")
            val result = nativeEncode(
                normalizedClaimIdHex,
                normalizedPlatform,
                normalizedAppId,
                buildNumber,
                issuedAtMs,
                expiresAtMs,
                normalizedLineageScope,
                normalizedNonceHex,
            )
            checkNotNull(result) { "nativeEncode returned null" }
            return result
        }

        private fun normalizePlatform(platform: String): String {
            val normalized = requireNonBlank(platform, "platform").lowercase()
            if (normalized == "apple" || normalized == "ios") return "Apple"
            if (normalized == "android") return "Android"
            throw IllegalArgumentException("platform must be either \"Apple\" or \"Android\"")
        }

        private fun requireNonBlank(value: String?, field: String): String {
            require(!value.isNullOrBlank()) { "$field must not be blank" }
            return value.trim()
        }

        private fun requireNonNegative(value: Long, field: String) {
            require(value >= 0) { "$field must not be negative" }
        }

        @JvmStatic
        private external fun nativeEncode(
            claimIdHex: String,
            platform: String,
            appId: String,
            buildNumber: Long,
            issuedAtMs: Long,
            expiresAtMs: Long,
            lineageScope: String,
            nonceHex: String,
        ): ByteArray
    }
}
