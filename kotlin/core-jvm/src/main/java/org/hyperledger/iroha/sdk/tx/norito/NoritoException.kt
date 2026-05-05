package org.hyperledger.iroha.sdk.tx.norito

class NoritoException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
