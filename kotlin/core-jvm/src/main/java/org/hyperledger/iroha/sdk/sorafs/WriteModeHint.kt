package org.hyperledger.iroha.sdk.sorafs

/**
 * Mirrors `sorafs_orchestrator::WriteModeHint`.
 *
 * Android callers can use this enum to request PQ-only upload paths when building gateway fetch
 * requests. The labels match the Norito JSON representation expected by the Rust orchestrator.
 */
enum class WriteModeHint(val label: String) {
    /** Default behaviour for read/replication workloads. */
    READ_ONLY("read_only"),
    /** Enforce PQ-only transport for upload workloads. */
    UPLOAD_PQ_ONLY("upload_pq_only");

    companion object {
        /**
         * Parse an incoming label. Returns `null` when the value does not map to a known hint.
         */
        @JvmStatic
        fun fromLabel(raw: String?): WriteModeHint? {
            if (raw == null) return null
            return when (raw.trim().lowercase().replace('-', '_')) {
                "read_only" -> READ_ONLY
                "upload_pq_only" -> UPLOAD_PQ_ONLY
                else -> null
            }
        }
    }
}
