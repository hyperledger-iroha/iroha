package org.hyperledger.iroha.sdk.subscriptions

/** Response payload returned after creating a subscription. */
class SubscriptionCreateResponse(
    @JvmField val ok: Boolean,
    @JvmField val subscriptionId: String,
    @JvmField val billingTriggerId: String,
    @JvmField val usageTriggerId: String?,
    @JvmField val firstChargeMs: Long,
    @JvmField val txHashHex: String,
)
