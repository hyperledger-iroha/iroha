package org.hyperledger.iroha.sdk.client

/** Immutable response wrapper for `GET /v1/identifier-policies`. */
class IdentifierPolicyListResponse(
    @JvmField val total: Long,
    items: List<IdentifierPolicySummary>,
) {
    @JvmField
    val items: List<IdentifierPolicySummary> = items.toList()
}
