package org.hyperledger.iroha.sdk.client

import java.util.Collections
import java.util.LinkedHashMap
import java.util.Optional

/** Helpers for validating Torii's metadata-only public pipeline status. */
internal object PipelineStatusExtractor {
    private val STATUS_KINDS =
        setOf("Queued", "Approved", "Committed", "Applied", "Rejected", "Expired")
    private val ROOT_FIELDS = setOf("hash", "status", "scope", "resolved_from")
    private val STATUS_FIELDS = setOf("kind", "block_height")

    /** Return the canonical top-level status kind, if present. */
    @JvmStatic
    fun extractStatusKind(payload: Any?): Optional<String> {
        if (payload !is Map<*, *>) {
            return Optional.empty()
        }
        val status = payload["status"] as? Map<*, *> ?: return Optional.empty()
        val kind = status["kind"] as? String ?: return Optional.empty()
        return if (kind in STATUS_KINDS) Optional.of(kind) else Optional.empty()
    }

    /** Reject retired detail fields and return a fresh metadata-only map. */
    @JvmStatic
    fun normalizePublicStatus(payload: Map<String, Any>?): Map<String, Any> {
        checkNotNull(payload) { "Pipeline status response must not be empty" }
        val rootKeys = payload.keys.toSet()
        check(rootKeys == ROOT_FIELDS) {
            val extras = (rootKeys - ROOT_FIELDS).sorted()
            val missing = (ROOT_FIELDS - rootKeys).sorted()
            when {
                extras.isNotEmpty() ->
                    "Pipeline status contains retired or unsupported fields: ${extras.joinToString(", ")}"
                else -> "Pipeline status is missing required fields: ${missing.joinToString(", ")}"
            }
        }
        val hash = payload["hash"] as? String
            ?: error("Pipeline status hash is missing or malformed")
        check(hash.matches(Regex("[0-9a-f]{64}"))) {
            "Pipeline status hash must be exact lowercase 32-byte hex"
        }
        val rawStatus = payload["status"] as? Map<*, *>
            ?: error("Pipeline status kind is missing or unsupported")
        val statusKeys = rawStatus.keys.map {
            it as? String ?: error("Pipeline status field names must be strings")
        }.toSet()
        check("kind" in statusKeys && statusKeys.all { it in STATUS_FIELDS }) {
            "Pipeline status contains retired, unsupported, or missing status fields"
        }
        val kind = rawStatus["kind"] as? String
            ?: error("Pipeline status kind is missing or unsupported")
        check(kind in STATUS_KINDS) { "Pipeline status kind is missing or unsupported" }
        val normalizedStatus = LinkedHashMap<String, Any>()
        normalizedStatus["kind"] = kind
        if ("block_height" in statusKeys) {
            val blockHeight = rawStatus["block_height"]
            check(hasPositiveBlockHeight(blockHeight)) {
                "Pipeline status block height must be a positive integer"
            }
            normalizedStatus["block_height"] = checkNotNull(blockHeight)
        }
        val scope = payload["scope"] as? String
            ?: error("Pipeline status scope is missing")
        check(scope in setOf("local", "auto", "global")) {
            "Pipeline status has an unsupported scope"
        }
        val resolvedFrom = payload["resolved_from"] as? String
            ?: error("Pipeline status resolution source is missing")
        check(resolvedFrom in setOf("queue", "cache", "state")) {
            "Pipeline status has an unsupported resolution source"
        }
        val normalized = LinkedHashMap<String, Any>()
        normalized["hash"] = hash
        normalized["status"] = Collections.unmodifiableMap(normalizedStatus)
        normalized["scope"] = scope
        normalized["resolved_from"] = resolvedFrom
        return Collections.unmodifiableMap(normalized)
    }

    /** Require exact global, state-backed terminal semantics for polling. */
    @JvmStatic
    fun requireAuthoritativeStatus(payload: Map<String, Any>?, expectedHash: String): String {
        val normalized = normalizePublicStatus(payload)
        check(normalized["hash"] == expectedHash) {
            "Pipeline status hash does not match the requested transaction hash"
        }
        check(normalized["scope"] == "global") { "Pipeline status must use global scope" }
        val status = normalized["status"] as Map<*, *>
        val kind = status["kind"] as String
        val resolvedFrom = normalized["resolved_from"] as String
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

    private fun hasPositiveBlockHeight(value: Any?): Boolean {
        val number = value as? Number ?: return false
        val double = number.toDouble()
        return double.isFinite() && double > 0.0 && double == kotlin.math.floor(double)
    }
}
