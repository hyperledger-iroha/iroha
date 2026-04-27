package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/refresh`. */
class OfflineCashRefreshRequest(
    val operationId: String,
    val lineageId: String,
    val accountId: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["operation_id"] = operationId
        map["lineage_id"] = lineageId
        map["account_id"] = accountId
        map["device_binding"] = deviceBinding.toJsonMap()
        map["device_proof"] = deviceProof.toJsonMap()
        return map
    }
}
