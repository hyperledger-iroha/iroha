package org.hyperledger.iroha.sdk.client

/** Raised when a transaction fails to reach a terminal status within the configured bounds. */
class TransactionTimeoutException(
    message: String,
    @JvmField val hashHex: String,
    @JvmField val attempts: Int,
    @JvmField val lastPayload: Map<String, Any>?,
) : RuntimeException(message)
