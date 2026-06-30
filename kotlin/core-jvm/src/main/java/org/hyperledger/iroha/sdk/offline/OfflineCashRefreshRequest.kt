package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/refresh`. */
class OfflineCashRefreshRequest(
    operationId: String,
    lineageId: String,
    accountId: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
) {
    val operationId: String = OfflineCashCodec.requireExactNonEmptyText(operationId, "operation_id")
    val lineageId: String = OfflineCashCodec.requireExactNonEmptyText(lineageId, "lineage_id")
    val accountId: String = OfflineCashCodec.requireExactNonEmptyText(accountId, "account_id")

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
