package org.hyperledger.iroha.sdk.sorafs

/**
 * Transport fallback ordering used by the SoraFS orchestrator.
 *
 * The enum mirrors the Rust `sorafs_orchestrator::TransportPolicy` so Android callers can
 * deterministically map between labels and policy variants when building fetch requests.
 */
enum class TransportPolicy(val label: String) {
    /** Prefer SoraNet relays, then QUIC, then Torii/HTTP, finally any vendor transport. */
    SORANET_FIRST("soranet-first"),
    /** Require SoraNet relays and fail rather than falling back to direct transports. */
    SORANET_STRICT("soranet-strict"),
    /** Restrict selection to direct transports (Torii/QUIC). */
    DIRECT_ONLY("direct-only");

    companion object {
        /**
         * Parse a policy label, accepting dash or underscore separated forms. Returns `null` when
         * the input does not match a known policy.
         */
        @JvmStatic
        fun fromLabel(raw: String?): TransportPolicy? {
            if (raw == null) return null
            return when (raw.trim().lowercase().replace('-', '_')) {
                "soranet_first" -> SORANET_FIRST
                "soranet_strict", "soranet_only" -> SORANET_STRICT
                "direct_only" -> DIRECT_ONLY
                else -> null
            }
        }
    }
}
