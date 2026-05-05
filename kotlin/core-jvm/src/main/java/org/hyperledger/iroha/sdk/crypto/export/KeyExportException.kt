package org.hyperledger.iroha.sdk.crypto.export

/** Raised when key export or import fails. */
class KeyExportException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
