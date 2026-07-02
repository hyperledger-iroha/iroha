package org.hyperledger.iroha.sdk.offline

/** Typed body for `POST /v1/offline/cash/setup`. */
class OfflineCashSetupRequest(
    accountId: String,
    assetDefinitionId: String,
    val deviceBinding: OfflineCashDeviceBinding,
    val deviceProof: OfflineCashDeviceProof,
) {
    val accountId: String = OfflineCashCodec.requireExactNonEmptyText(accountId, "account_id")
    val assetDefinitionId: String = OfflineCashCodec.requireExactNonEmptyText(
        assetDefinitionId,
        "asset_definition_id",
    )

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["account_id"] = accountId
        map["asset_definition_id"] = assetDefinitionId
        map["device_binding"] = deviceBinding.toJsonMap()
        map["device_proof"] = deviceProof.toJsonMap()
        return map
    }
}
