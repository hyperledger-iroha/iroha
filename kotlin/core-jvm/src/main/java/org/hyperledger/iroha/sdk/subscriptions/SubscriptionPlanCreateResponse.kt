package org.hyperledger.iroha.sdk.subscriptions

/** Response payload returned after registering a subscription plan. */
class SubscriptionPlanCreateResponse(
    @JvmField val ok: Boolean,
    @JvmField val planId: String,
    @JvmField val txHashHex: String,
)
