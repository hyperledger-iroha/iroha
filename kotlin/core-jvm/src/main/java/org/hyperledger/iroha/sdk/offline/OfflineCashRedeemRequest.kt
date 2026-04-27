package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/redeem`. */
class OfflineCashRedeemRequest(
    val operationId: String,
    val lineageId: String,
    val accountId: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
    val amount: String,
    receipts: List<OfflineTransferReceipt>,
    val redeemProof: OfflineRedeemRequestProof,
) {
    private val _receipts: List<OfflineTransferReceipt> = receipts.toList()

    val receipts: List<OfflineTransferReceipt> get() = _receipts.toList()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["operation_id"] = operationId
        map["lineage_id"] = lineageId
        map["account_id"] = accountId
        map["device_binding"] = deviceBinding.toJsonMap()
        map["device_proof"] = deviceProof.toJsonMap()
        map["amount"] = amount
        map["receipts"] = _receipts.map { it.toJsonMap() }
        map["redeem_proof"] = redeemProof.toJsonMap()
        return map
    }
}
