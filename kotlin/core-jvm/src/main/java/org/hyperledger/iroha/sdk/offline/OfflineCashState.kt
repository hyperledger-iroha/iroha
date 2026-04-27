package org.hyperledger.iroha.sdk.offline

/** Lineage state snapshot returned by every cash mutation route. */
class OfflineCashState(
    val lineageId: String,
    val accountId: String,
    val deviceId: String,
    val offlinePublicKey: String,
    val assetDefinitionId: String,
    val balance: String,
    val lockedBalance: String,
    val serverRevision: Long,
    val serverStateHash: String,
    val pendingLocalRevision: Long,
    val authorization: OfflineSpendAuthorization,
    val issuerSignatureBase64: String,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["lineage_id"] = lineageId
        map["account_id"] = accountId
        map["device_id"] = deviceId
        map["offline_public_key"] = offlinePublicKey
        map["asset_definition_id"] = assetDefinitionId
        map["balance"] = balance
        map["locked_balance"] = lockedBalance
        map["server_revision"] = serverRevision
        map["server_state_hash"] = serverStateHash
        map["pending_local_revision"] = pendingLocalRevision
        map["authorization"] = authorization.toJsonMap()
        map["issuer_signature_base64"] = issuerSignatureBase64
        return map
    }
}
