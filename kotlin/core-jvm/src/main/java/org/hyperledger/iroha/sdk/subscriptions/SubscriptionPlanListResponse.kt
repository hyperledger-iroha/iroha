package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Paginated list of subscription plans. */
class SubscriptionPlanListResponse(
    items: List<SubscriptionPlanListItem>,
    @JvmField val total: Long,
) {
    @JvmField val items: List<SubscriptionPlanListItem> = items.toList()

    /** Subscription plan list item. */
    class SubscriptionPlanListItem(
        @JvmField val planId: String,
        plan: Map<String, Any>,
    ) {
        private val _plan: Map<String, Any> = LinkedHashMap(plan)

        val plan: Map<String, Any> get() = _plan

        fun planJson(): String = JsonEncoder.encode(_plan)
    }
}
