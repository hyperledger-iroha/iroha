package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonParser

/** Immutable view over `/v1/offline/allowances` responses. */
class OfflineAllowanceList(
    items: List<OfflineAllowanceItem>,
    val total: Long,
) {
    private val _items: List<OfflineAllowanceItem> = items.toList()

    val items: List<OfflineAllowanceItem> get() = _items
}

class OfflineAllowanceItem(
    val certificateIdHex: String,
    val controllerId: String,
    val controllerDisplay: String,
    val assetId: String,
    val assetDefinitionId: String,
    val assetDefinitionName: String,
    val assetDefinitionAlias: String?,
    val registeredAtMs: Long,
    val certificateExpiresAtMs: Long,
    val policyExpiresAtMs: Long,
    val refreshAtMs: Long?,
    val verdictIdHex: String?,
    val attestationNonceHex: String?,
    val remainingAmount: String?,
    /** Raw Norito JSON of the registered certificate record. */
    val recordJson: String?,
) {
    /**
     * Parses the raw record JSON into an immutable map representation. The parser mirrors the
     * minimal JSON support bundled with the SDK.
     *
     * @throws IllegalStateException if the JSON payload is malformed or not an object
     */
    @Suppress("UNCHECKED_CAST")
    fun recordAsMap(): Map<String, Any> {
        if (recordJson.isNullOrBlank()) return emptyMap()
        val parsed = JsonParser.parse(recordJson)
        check(parsed is Map<*, *>) { "record is not a JSON object" }
        return (parsed as Map<String, Any>).toMap()
    }
}
