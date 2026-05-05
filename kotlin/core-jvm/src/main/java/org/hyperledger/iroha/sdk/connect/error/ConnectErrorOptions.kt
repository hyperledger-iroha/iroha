package org.hyperledger.iroha.sdk.connect.error

/** Optional overrides when mapping errors to the taxonomy. */
data class ConnectErrorOptions(
    @JvmField val fatal: Boolean?,
    @JvmField val httpStatus: Int?,
) {
    companion object {
        @JvmStatic
        fun empty(): ConnectErrorOptions = ConnectErrorOptions(null, null)
    }
}
