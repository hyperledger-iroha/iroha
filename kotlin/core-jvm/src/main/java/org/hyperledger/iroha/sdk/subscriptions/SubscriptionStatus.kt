package org.hyperledger.iroha.sdk.subscriptions

/** Supported subscription status literals accepted by Torii list filters. */
enum class SubscriptionStatus(val slug: String) {
    ACTIVE("active"),
    PAUSED("paused"),
    PAST_DUE("past_due"),
    CANCELED("canceled"),
    SUSPENDED("suspended"),
}
