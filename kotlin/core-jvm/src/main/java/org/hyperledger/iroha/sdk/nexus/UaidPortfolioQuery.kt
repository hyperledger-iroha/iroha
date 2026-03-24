package org.hyperledger.iroha.sdk.nexus

/** Query parameters for `/v1/accounts/{uaid}/portfolio`. */
class UaidPortfolioQuery(
    val assetId: String? = null,
) {
    fun toQueryParameters(): Map<String, String> = buildMap {
        assetId?.trim()?.takeIf { it.isNotEmpty() }?.let { put("asset_id", it) }
    }
}
