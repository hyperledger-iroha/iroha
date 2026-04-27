package org.hyperledger.iroha.sdk.offline

/** Unified response envelope returned by every cash mutation route under `/v1/offline/cash/`. */
class OfflineCashEnvelope(
    val lineageState: OfflineCashState,
    val settlement: OfflineMutationSettlement?,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["lineage_state"] = lineageState.toJsonMap()
        if (settlement != null) map["settlement"] = settlement.toJsonMap()
        return map
    }
}
