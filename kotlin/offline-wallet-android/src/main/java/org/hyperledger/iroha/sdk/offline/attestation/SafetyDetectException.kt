package org.hyperledger.iroha.sdk.offline.attestation

/** Runtime exception thrown when Safety Detect attestation fails. */
class SafetyDetectException : RuntimeException {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable?) : super(message, cause)

    companion object {
        private const val serialVersionUID: Long = 1L
    }
}
