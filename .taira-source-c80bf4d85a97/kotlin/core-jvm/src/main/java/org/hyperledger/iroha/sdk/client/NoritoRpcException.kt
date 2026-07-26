package org.hyperledger.iroha.sdk.client

/** Exception thrown when Norito RPC requests fail or cannot be executed. */
class NoritoRpcException : RuntimeException {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
