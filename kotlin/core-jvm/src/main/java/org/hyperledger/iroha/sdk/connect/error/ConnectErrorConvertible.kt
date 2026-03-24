package org.hyperledger.iroha.sdk.connect.error

/** Types that can be converted into [ConnectError]. */
interface ConnectErrorConvertible {
    fun toConnectError(): ConnectError
}
