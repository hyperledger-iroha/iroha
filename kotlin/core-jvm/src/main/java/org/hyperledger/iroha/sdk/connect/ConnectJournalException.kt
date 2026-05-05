package org.hyperledger.iroha.sdk.connect

/** Exception raised when Connect queue journals encounter IO or corruption errors. */
class ConnectJournalException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)

    companion object {
        private const val serialVersionUID = 1L
    }
}
