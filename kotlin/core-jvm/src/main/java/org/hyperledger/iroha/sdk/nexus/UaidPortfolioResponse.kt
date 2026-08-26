package org.hyperledger.iroha.sdk.nexus

import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/** Immutable view over `/v1/accounts/{uaid}/portfolio` responses. */
class UaidPortfolioResponse(
    @JvmField val uaid: String,
    @JvmField val totals: UaidPortfolioTotals,
    dataspaces: List<UaidPortfolioDataspace>,
) {
    init {
        UaidLiteral.canonicalize(uaid, "uaid")
    }

    @JvmField val dataspaces: List<UaidPortfolioDataspace> = dataspaces.toList()

    /** Aggregated account/position totals for a UAID. */
    class UaidPortfolioTotals(
        @JvmField val accounts: Long,
        @JvmField val positions: Long,
    ) {
        init {
            require(accounts >= 0) { "accounts must be non-negative" }
            require(positions >= 0) { "positions must be non-negative" }
        }
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
    ) {
        init {
            KotodamaQuantity.parseCanonical(quantity)
        }
    }
}
