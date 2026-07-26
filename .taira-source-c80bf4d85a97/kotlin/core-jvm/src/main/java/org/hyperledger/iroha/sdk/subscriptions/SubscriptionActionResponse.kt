package org.hyperledger.iroha.sdk.subscriptions

/** Response payload returned after subscription actions (pause/resume/cancel/charge-now/usage). */
class SubscriptionActionResponse(
    @JvmField val ok: Boolean,
    @JvmField val subscriptionId: String,
    @JvmField val txHashHex: String,
)
