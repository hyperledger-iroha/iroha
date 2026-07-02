package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/redeem`. */
class OfflineCashRedeemRequest(
    operationId: String,
    lineageId: String,
    accountId: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
    amount: String,
    receipts: List<OfflineTransferReceipt>,
    val redeemProof: OfflineRedeemRequestProof,
) {
    private val _receipts: List<OfflineTransferReceipt> = receipts.toList()

    val operationId: String = OfflineCashCodec.requireExactNonEmptyText(operationId, "operation_id")
    val lineageId: String = OfflineCashCodec.requireExactNonEmptyText(lineageId, "lineage_id")
    val accountId: String = OfflineCashCodec.requireExactNonEmptyText(accountId, "account_id")
    val amount: String = OfflineCashCodec.canonicalNonNegativeAmountString(amount, "amount")
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
