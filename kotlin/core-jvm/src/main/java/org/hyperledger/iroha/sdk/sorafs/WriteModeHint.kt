package org.hyperledger.iroha.sdk.sorafs

/**
 * Mirrors `sorafs_orchestrator::WriteModeHint`.
 *
 * Android callers can use this enum to request PQ-only upload paths when building gateway fetch
 * requests. The labels match the Norito JSON representation expected by the Rust orchestrator.
 */
enum class WriteModeHint(val label: String) {
    /** Default behaviour for read/replication workloads. */
    READ_ONLY("read-only"),
    /** Enforce PQ-only transport for upload workloads. */
    UPLOAD_PQ_ONLY("upload-pq-only");

    companion object {
        /**
         * Parse one exact canonical V1 label. Returns `null` for every alias or unknown value.
         */
        @JvmStatic
        fun fromLabel(raw: String?): WriteModeHint? {
            return when (raw) {
                "read-only" -> READ_ONLY
                "upload-pq-only" -> UPLOAD_PQ_ONLY
                else -> null
            }
        }
    }
}
