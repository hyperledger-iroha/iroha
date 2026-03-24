package org.hyperledger.iroha.sdk.subscriptions

/** Supported subscription status literals accepted by Torii list filters. */
enum class SubscriptionStatus(val slug: String) {
    ACTIVE("active"),
    PAUSED("paused"),
    PAST_DUE("past_due"),
    CANCELED("canceled"),
    SUSPENDED("suspended");

    companion object {
        @JvmStatic
        fun fromString(value: String): SubscriptionStatus {
            val normalized = value.trim().lowercase()
            require(normalized.isNotEmpty()) { "status must not be blank" }
            return entries.find { it.slug == normalized }
                ?: throw IllegalArgumentException("unknown subscription status: $value")
        }
    }
}
