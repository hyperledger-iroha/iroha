package org.hyperledger.iroha.sdk.sorafs

/**
 * Staged anonymity roll-out policy for SoraNet fetches.
 *
 * This mirrors `sorafs_orchestrator::AnonymityPolicy` and is used when serialising gateway
 * fetch requests or telemetry overrides.
 */
enum class AnonymityPolicy(val label: String) {
    /** Require at least one PQ-capable guard in the pinned relay set. */
    ANON_GUARD_PQ("anon-guard-pq"),
    /** Require PQ coverage on the majority of SoraNet hops. */
    ANON_MAJORITY_PQ("anon-majority-pq"),
    /** Enforce PQ-only SoraNet paths and reject direct transport substitution. */
    ANON_STRICT_PQ("anon-strict-pq");

    companion object {
        /**
         * Parse one exact canonical V1 policy label. Returns `null` for every alias or unknown
         * value.
         */
        @JvmStatic
        fun fromLabel(raw: String?): AnonymityPolicy? {
            return when (raw) {
                "anon-guard-pq" -> ANON_GUARD_PQ
                "anon-majority-pq" -> ANON_MAJORITY_PQ
                "anon-strict-pq" -> ANON_STRICT_PQ
                else -> null
            }
        }
    }
}
