package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/load`. */
class OfflineCashLoadRequest(
    val operationId: String,
    val lineageId: String?,
    val accountId: String,
    val assetDefinitionId: String,
    val amount: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["operation_id"] = operationId
        if (lineageId != null) map["lineage_id"] = lineageId
        map["account_id"] = accountId
        map["asset_definition_id"] = assetDefinitionId
        map["amount"] = amount
        map["device_binding"] = deviceBinding.toJsonMap()
        map["device_proof"] = deviceProof.toJsonMap()
        return map
    }
}
