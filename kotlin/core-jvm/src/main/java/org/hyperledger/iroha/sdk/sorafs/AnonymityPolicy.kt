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
    ANON_MAJORIY_PQ("anon-majority-pq"),
    /** Enforce PQ-only SoraNet paths; fall back to direct transports when unavailable. */
    ANON_STRICT_PQ("anon-strict-pq");

    companion object {
        /**
         * Parse a policy label, accepting the canonical hyphenated names plus stage aliases. Returns
         * `null` for unknown values.
         */
        @JvmStatic
        fun fromLabel(raw: String?): AnonymityPolicy? {
            if (raw == null) return null
            return when (raw.trim().lowercase()) {
                "anon_guard_pq", "anon-guard-pq", "stage_a", "stage-a", "stagea" -> ANON_GUARD_PQ
                "anon_majority_pq", "anon-majority-pq", "stage_b", "stage-b", "stageb" -> ANON_MAJORIY_PQ
                "anon_strict_pq", "anon-strict-pq", "stage_c", "stage-c", "stagec" -> ANON_STRICT_PQ
                else -> null
            }
        }
    }
}
