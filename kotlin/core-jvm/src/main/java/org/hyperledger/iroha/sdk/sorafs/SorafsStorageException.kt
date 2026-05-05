package org.hyperledger.iroha.sdk.sorafs

/** Exception raised when SoraFS storage interactions fail. */
class SorafsStorageException : RuntimeException {
    internal constructor(message: String) : super(message)
    internal constructor(message: String, cause: Throwable) : super(message, cause)
}
