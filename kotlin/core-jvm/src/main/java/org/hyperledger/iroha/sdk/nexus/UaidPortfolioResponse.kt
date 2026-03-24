package org.hyperledger.iroha.sdk.nexus

/** Immutable view over `/v1/accounts/{uaid}/portfolio` responses. */
class UaidPortfolioResponse(
    @JvmField val uaid: String,
    @JvmField val totals: UaidPortfolioTotals,
    dataspaces: List<UaidPortfolioDataspace>,
) {
    @JvmField val dataspaces: List<UaidPortfolioDataspace> = dataspaces.toList()

    /** Aggregated account/position totals for a UAID. */
    class UaidPortfolioTotals(
        accounts: Long,
        positions: Long,
    ) {
        @JvmField val accounts: Long = maxOf(0L, accounts)
        @JvmField val positions: Long = maxOf(0L, positions)
    }

    /** Dataspace entry within the aggregated portfolio response. */
    class UaidPortfolioDataspace(
        @JvmField val dataspaceId: Long,
        @JvmField val dataspaceAlias: String?,
        accounts: List<UaidPortfolioAccount>,
    ) {
        @JvmField val accounts: List<UaidPortfolioAccount> = accounts.toList()
    }

    /** Account entry underneath a dataspace portfolio record. */
    class UaidPortfolioAccount(
        @JvmField val accountId: String,
        @JvmField val label: String?,
        assets: List<UaidPortfolioAsset>,
    ) {
        @JvmField val assets: List<UaidPortfolioAsset> = assets.toList()
    }

    /** Asset balance entry associated with an account. */
    class UaidPortfolioAsset(
        @JvmField val assetId: String,
        @JvmField val assetDefinitionId: String,
        @JvmField val quantity: String,
    )
}
