package org.hyperledger.iroha.sdk.offline

/** Policy bounds for an offline wallet certificate. */
class OfflineWalletPolicy(
    val maxBalance: String,
    val maxTxValue: String,
    val expiresAtMs: Long,
) {
    internal fun toJsonMap(): Map<String, Any> {
        val map = LinkedHashMap<String, Any>()
        map["max_balance"] = maxBalance
        map["max_tx_value"] = maxTxValue
        map["expires_at_ms"] = expiresAtMs
        return map
    }
}
