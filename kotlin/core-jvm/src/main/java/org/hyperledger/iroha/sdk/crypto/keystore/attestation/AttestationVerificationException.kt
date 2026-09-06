package org.hyperledger.iroha.sdk.crypto.keystore.attestation

/**
 * Raised when attestation material cannot be validated or does not match the caller's policy.
 */
class AttestationVerificationException : Exception {

    constructor(message: String) : super(message)

    constructor(message: String, cause: Throwable) : super(message, cause)

    companion object {
        private const val serialVersionUID: Long = 1L
    }
}
