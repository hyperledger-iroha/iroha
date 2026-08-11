package org.hyperledger.iroha.sdk.client

/** Raised when a transaction reports a terminal failure status. */
class TransactionStatusException(
    @JvmField val hashHex: String,
    @JvmField val status: String,
    @JvmField val payload: Map<String, Any>?,
) : RuntimeException("Transaction $hashHex reported failure status $status")
