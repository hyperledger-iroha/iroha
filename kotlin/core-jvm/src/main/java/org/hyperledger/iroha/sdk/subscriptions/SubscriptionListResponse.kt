package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Paginated list of subscriptions. */
class SubscriptionListResponse(
    items: List<SubscriptionRecord>,
    @JvmField val total: Long,
) {
    @JvmField val items: List<SubscriptionRecord> = items.toList()

    /** Subscription list entry (also returned by GET /v1/subscriptions/{id}). */
    class SubscriptionRecord(
        @JvmField val subscriptionId: String,
        subscription: Map<String, Any>,
        invoice: Map<String, Any>?,
        plan: Map<String, Any>?,
    ) {
        private val _subscription: Map<String, Any> = LinkedHashMap(subscription)
        private val _invoice: Map<String, Any>? = invoice?.let { LinkedHashMap(it) }
        private val _plan: Map<String, Any>? = plan?.let { LinkedHashMap(it) }

        val subscription: Map<String, Any> get() = _subscription
        val invoice: Map<String, Any>? get() = _invoice
        val plan: Map<String, Any>? get() = _plan

        fun subscriptionJson(): String = JsonEncoder.encode(_subscription)
        fun invoiceJson(): String? = _invoice?.let { JsonEncoder.encode(it) }
        fun planJson(): String? = _plan?.let { JsonEncoder.encode(it) }
    }
}
