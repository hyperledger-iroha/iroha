package org.hyperledger.iroha.sdk.nexus

/** Query parameters for `/v1/accounts/{uaid}/portfolio`. */
class UaidPortfolioQuery(
    val assetId: String? = null,
) {
    fun toQueryParameters(): Map<String, String> = buildMap {
        assetId?.let { put("asset_id", requireExactNonEmpty(it, "assetId")) }
    }

    private companion object {
        fun requireExactNonEmpty(value: String, field: String): String {
            require(value.isNotEmpty()) { "$field must not be blank" }
            require(value.trim() == value) { "$field must not contain surrounding whitespace" }
            return value
        }
    }
}
