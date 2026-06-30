package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/load`. */
class OfflineCashLoadRequest(
    operationId: String,
    lineageId: String?,
    accountId: String,
    assetDefinitionId: String,
    amount: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
) {
    val operationId: String = OfflineCashCodec.requireExactNonEmptyText(operationId, "operation_id")
    val lineageId: String? = OfflineCashCodec.requireOptionalExactNonEmptyText(lineageId, "lineage_id")
    val accountId: String = OfflineCashCodec.requireExactNonEmptyText(accountId, "account_id")
    val assetDefinitionId: String = OfflineCashCodec.requireExactNonEmptyText(
        assetDefinitionId,
        "asset_definition_id",
    )
    val amount: String = OfflineCashCodec.canonicalNonNegativeAmountString(amount, "amount")

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
