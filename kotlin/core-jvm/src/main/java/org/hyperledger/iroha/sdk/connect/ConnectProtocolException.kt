package org.hyperledger.iroha.sdk.connect

/** Raised when a Connect frame/envelope cannot be parsed or encoded. */
class ConnectProtocolException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
