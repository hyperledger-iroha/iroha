package org.hyperledger.iroha.sdk.client

import java.util.Optional

/** Helpers for parsing Torii pipeline status payloads. */
internal object PipelineStatusExtractor {
    private val STATUS_KINDS =
        setOf("Queued", "Approved", "Committed", "Applied", "Rejected", "Expired")
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
    fun requireAuthoritativeStatus(payload: Map<String, Any>?, expectedHash: String): String {
        checkNotNull(payload) { "Pipeline status response must not be empty" }
        check(payload["hash"] == expectedHash) {
            "Pipeline status hash does not match the requested transaction hash"
        }
        check(payload["scope"] == "global") { "Pipeline status must use global scope" }
        check(payload["summary"] is String) { "Pipeline status summary is missing or malformed" }
        val status = payload["status"] as? Map<*, *>
            ?: error("Pipeline status kind is missing or unsupported")
        val kind = status["kind"] as? String
            ?: error("Pipeline status kind is missing or unsupported")
        check(kind in STATUS_KINDS) { "Pipeline status kind is missing or unsupported" }
        val resolvedFrom = payload["resolved_from"] as? String
            ?: error("Pipeline status resolution source is missing")
        when (kind) {
            "Applied" -> {
                check(resolvedFrom == "state" && hasPositiveBlockHeight(status["block_height"])) {
                    "Applied pipeline status must be state-resolved with a positive block height"
                }
            }
            "Rejected", "Expired" -> check(resolvedFrom == "state") {
                "Terminal pipeline failure must be resolved from state"
            }
            else -> check(resolvedFrom in setOf("queue", "cache", "state")) {
                "Pipeline status has an unsupported resolution source"
            }
        }
        return kind
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
        val details = record["details"]
        if (details is Map<*, *>) {
            for (key in REJECTION_REASON_KEYS) {
                val reason = coerceReason(details[key])
                if (reason.isPresent) {
                    return reason
                }
            }
        }
        return Optional.empty()
    }

    private fun hasPositiveBlockHeight(value: Any?): Boolean {
        val number = value as? Number ?: return false
        val double = number.toDouble()
        return double.isFinite() && double > 0.0 && double == kotlin.math.floor(double)
    }
}
