package org.hyperledger.iroha.sdk.sorafs

/**
 * Helper for composing SoraFS gateway fetch options.
 *
 * The builder exposes the subset of orchestrator configuration that Android clients commonly
 * tweak (telemetry labels, provider limits, retry budgets, transport/anonymity policy). The helper
 * serialises to a map matching the Norito JSON structure consumed by the Rust orchestrator.
 */
class GatewayFetchOptions(
    manifestEnvelopeBase64: String? = null,
    manifestCidHex: String? = null,
    clientId: String? = null,
    telemetryRegion: String? = null,
    rolloutPhase: String? = null,
    maxPeers: Int? = null,
    retryBudget: Int? = null,
    val transportPolicy: TransportPolicy = TransportPolicy.SORANET_FIRST,
    val anonymityPolicy: AnonymityPolicy = AnonymityPolicy.ANON_GUARD_PQ,
    val writeModeHint: WriteModeHint = WriteModeHint.READ_ONLY,
) {
    val manifestEnvelopeBase64: String? = manifestEnvelopeBase64?.trim()
        ?.takeIf { it.isNotEmpty() }
        ?.let { SorafsInputValidator.normalizeBase64MaybeUrl(it, "manifestEnvelopeBase64") }

    val manifestCidHex: String? = manifestCidHex?.trim()
        ?.takeIf { it.isNotEmpty() }
        ?.let { SorafsInputValidator.normalizeHex(it, "manifestCidHex") }

    val clientId: String? = emptyToNull(clientId)
    val telemetryRegion: String? = emptyToNull(telemetryRegion)
    val rolloutPhase: String? = emptyToNull(rolloutPhase)

    val maxPeers: Int? = maxPeers?.also {
        require(it >= 1) { "maxPeers must be greater than zero" }
    }

    val retryBudget: Int? = retryBudget?.also {
        require(it >= 0) { "retryBudget must be non-negative" }
    }

    /**
     * Serialise the options to a JSON-compatible map. Only explicitly configured entries are
     * included.
     */
    fun toJson(): Map<String, Any> = buildMap {
        this@GatewayFetchOptions.manifestEnvelopeBase64?.let { put("manifest_envelope_b64", it) }
        this@GatewayFetchOptions.manifestCidHex?.let { put("manifest_cid_hex", it) }
        this@GatewayFetchOptions.clientId?.let { put("client_id", it) }
        this@GatewayFetchOptions.telemetryRegion?.let { put("telemetry_region", it) }
        this@GatewayFetchOptions.rolloutPhase?.let { put("rollout_phase", it) }
        this@GatewayFetchOptions.maxPeers?.let { put("max_peers", it) }
        this@GatewayFetchOptions.retryBudget?.let { put("retry_budget", it) }
        put("transport_policy", transportPolicy.label)
        put("anonymity_policy", anonymityPolicy.label)
        if (writeModeHint != WriteModeHint.READ_ONLY) {
            put("write_mode_hint", writeModeHint.label)
        }
    }
}

private fun emptyToNull(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
