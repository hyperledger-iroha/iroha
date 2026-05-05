package org.hyperledger.iroha.sdk.client

import java.util.Optional

/** Helpers for parsing Torii pipeline status payloads. */
internal object PipelineStatusExtractor {
    private val REJECTION_REASON_KEYS =
        arrayOf("rejection_reason", "rejectionReason", "reason", "reject_code", "rejectCode")

    @JvmStatic
    fun extractStatusKind(payload: Any?): Optional<String> {
        if (payload !is Map<*, *>) {
            return Optional.empty()
        }
        val direct = coerceStatus(payload["status"])
        if (direct.isPresent) {
            return direct
        }
        val content = payload["content"]
        if (content is Map<*, *>) {
            return coerceStatus(content["status"])
        }
        return Optional.empty()
    }

    @JvmStatic
    fun extractRejectionReason(payload: Any?): Optional<String> {
        if (payload !is Map<*, *>) {
            return Optional.empty()
        }
        val direct = coerceReasonFromRecord(payload)
        if (direct.isPresent) {
            return direct
        }
        val content = payload["content"]
        if (content is Map<*, *>) {
            val contentReason = coerceReasonFromRecord(content)
            if (contentReason.isPresent) {
                return contentReason
            }
            val status = content["status"]
            if (status is Map<*, *>) {
                val statusReason = coerceReasonFromRecord(status)
                if (statusReason.isPresent) {
                    return statusReason
                }
                if ("Rejected".equals(status["kind"]?.toString(), ignoreCase = true)) {
                    return coerceReason(status["content"])
                }
            }
        }
        return Optional.empty()
    }

    private fun coerceStatus(status: Any?): Optional<String> {
        if (status is Map<*, *>) {
            val kind = status["kind"]
            if (kind != null) {
                return Optional.of(kind.toString())
            }
        } else if (status != null) {
            return Optional.of(status.toString())
        }
        return Optional.empty()
    }

    private fun coerceReason(reason: Any?): Optional<String> {
        if (reason == null) {
            return Optional.empty()
        }
        val text = reason.toString().trim()
        if (text.isEmpty()) {
            return Optional.empty()
        }
        return Optional.of(text)
    }

    private fun coerceReasonFromRecord(record: Map<*, *>): Optional<String> {
        for (key in REJECTION_REASON_KEYS) {
            val reason = coerceReason(record[key])
            if (reason.isPresent) {
                return reason
            }
        }
        return Optional.empty()
    }
}
