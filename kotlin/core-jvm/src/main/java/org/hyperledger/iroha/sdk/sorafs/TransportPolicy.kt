package org.hyperledger.iroha.sdk.sorafs

/**
 * Transport selection ordering used by the SoraFS orchestrator.
 *
 * The enum mirrors the Rust `sorafs_orchestrator::TransportPolicy` so Android callers can
 * deterministically map between labels and policy variants when building fetch requests.
 */
enum class TransportPolicy(val label: String) {
    /** Prefer SoraNet relays, then QUIC, then Torii/HTTP, finally any vendor transport. */
    SORANET_FIRST("soranet-first"),
    /** Require SoraNet relays and fail rather than selecting direct transports. */
    SORANET_STRICT("soranet-strict"),
    /** Restrict selection to direct transports (Torii/QUIC). */
    DIRECT_ONLY("direct-only");

    companion object {
        /**
         * Parse one exact canonical V1 policy label. Returns `null` when the input does not match a
         * known policy byte-for-byte.
         */
        @JvmStatic
        fun fromLabel(raw: String?): TransportPolicy? {
            return when (raw) {
                "soranet-first" -> SORANET_FIRST
                "soranet-strict" -> SORANET_STRICT
                "direct-only" -> DIRECT_ONLY
                else -> null
            }
        }
    }
}
