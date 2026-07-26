package org.hyperledger.iroha.sdk.crypto

/**
 * Raised when a key provider cannot complete a requested operation.
 *
 * This exception is surfaced to callers so they can decide whether to retry with a different
 * security preference or surface a user prompt. Providers should include actionable messages.
 */
class KeyManagementException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
