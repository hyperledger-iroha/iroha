package org.hyperledger.iroha.sdk.crypto

/** Raised when signing cannot complete (missing algorithm support, invalid key material, etc.). */
class SigningException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
