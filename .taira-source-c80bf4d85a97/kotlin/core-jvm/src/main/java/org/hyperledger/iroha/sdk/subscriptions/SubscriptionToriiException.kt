package org.hyperledger.iroha.sdk.subscriptions

/** Runtime error raised when subscription Torii endpoints reject a request or parsing fails. */
class SubscriptionToriiException : RuntimeException {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
