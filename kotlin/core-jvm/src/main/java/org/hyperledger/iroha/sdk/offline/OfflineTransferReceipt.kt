package org.hyperledger.iroha.sdk.offline

/**
 * Signed peer-to-peer transfer receipt accompanying sync and redeem requests.
 *
 * The `deviceProof` field serializes as `device_proof` on the wire. Torii's app API
 * translates that key into the internal `OfflineDeviceAttestation` struct on ingress
 * (see `translate_cash_receipt_to_lineage_json` at
 * `crates/iroha_torii/src/lib.rs:4921-4954`); clients must not emit `attestation`
 * directly.
 */
class OfflineTransferReceipt(
    val version: Int,
    val transferId: String,
    val direction: String,
    val lineageId: String,
    val accountId: String,
    val deviceId: String,
    val offlinePublicKey: String,
    val preBalance: String,
    val postBalance: String,
    val preLockedBalance: String,
    val postLockedBalance: String,
    val preStateHash: String,
    val postStateHash: String,
    val localRevision: Long,
    val counterpartyLineageId: String,
    val counterpartyAccountId: String,
    val counterpartyDeviceId: String,
    val counterpartyOfflinePublicKey: String,
    val amount: String,
    val authorization: OfflineSpendAuthorization? = null,
    val deviceProof: OfflineCashDeviceProof,
    val sourcePayload: String? = null,
    val senderSignatureBase64: String,
    val createdAtMs: Long,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["version"] = version.toLong()
        map["transfer_id"] = transferId
        map["direction"] = direction
        map["lineage_id"] = lineageId
        map["account_id"] = accountId
        map["device_id"] = deviceId
        map["offline_public_key"] = offlinePublicKey
        map["pre_balance"] = preBalance
        map["post_balance"] = postBalance
        map["pre_locked_balance"] = preLockedBalance
        map["post_locked_balance"] = postLockedBalance
        map["pre_state_hash"] = preStateHash
        map["post_state_hash"] = postStateHash
        map["local_revision"] = localRevision
        map["counterparty_lineage_id"] = counterpartyLineageId
        map["counterparty_account_id"] = counterpartyAccountId
        map["counterparty_device_id"] = counterpartyDeviceId
        map["counterparty_offline_public_key"] = counterpartyOfflinePublicKey
        map["amount"] = amount
        if (authorization != null) map["authorization"] = authorization.toJsonMap()
        map["device_proof"] = deviceProof.toJsonMap()
        if (sourcePayload != null) map["source_payload"] = sourcePayload
        map["sender_signature_base64"] = senderSignatureBase64
        map["created_at_ms"] = createdAtMs
        return map
    }
}
