package org.hyperledger.iroha.sdk.client

/** Immutable response wrapper for `GET /v1/ram-lfe/program-policies`. */
class RamLfeProgramPolicyListResponse(
    @JvmField val total: Long,
    items: List<RamLfeProgramPolicySummary>,
) {
    @JvmField
    val items: List<RamLfeProgramPolicySummary> = items.toList()
}
