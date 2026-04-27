package org.hyperledger.iroha.sdk.offline

/**
 * Signed spend-limit authorization issued by Torii for an offline lineage.
 *
 * `deviceId`, `offlinePublicKey`, and `appAttestKeyId` are absent from Torii's cash-wire
 * shape (Torii strips them on response and reconstitutes them from `deviceBinding` on
 * request, see `crates/iroha_torii/src/lib.rs:4777-4832`). They are modeled here as
 * nullable and omitted from `toJsonMap()` when absent, so the same class round-trips the
 * cash shape without carrying internal lineage fields.
 */
class OfflineSpendAuthorization(
    val authorizationId: String,
    val lineageId: String,
    val accountId: String,
    val deviceId: String?,
    val offlinePublicKey: String?,
    val verdictId: String,
    val maxBalance: String,
    val maxTxValue: String,
    val issuedAtMs: Long,
    val refreshAtMs: Long,
    val expiresAtMs: Long,
    val deviceBinding: OfflineCashDeviceBinding? = null,
    val appAttestKeyId: String?,
    val issuerSignatureBase64: String,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["authorization_id"] = authorizationId
        map["lineage_id"] = lineageId
        map["account_id"] = accountId
        if (deviceId != null) map["device_id"] = deviceId
        if (offlinePublicKey != null) map["offline_public_key"] = offlinePublicKey
        map["verdict_id"] = verdictId
        map["max_balance"] = maxBalance
        map["max_tx_value"] = maxTxValue
        map["issued_at_ms"] = issuedAtMs
        map["refresh_at_ms"] = refreshAtMs
        map["expires_at_ms"] = expiresAtMs
        if (deviceBinding != null) map["device_binding"] = deviceBinding.toJsonMap()
        if (appAttestKeyId != null) map["app_attest_key_id"] = appAttestKeyId
        map["issuer_signature_base64"] = issuerSignatureBase64
        return map
    }
}
