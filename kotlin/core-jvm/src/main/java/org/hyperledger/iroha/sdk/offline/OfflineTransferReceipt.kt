package org.hyperledger.iroha.sdk.offline

import java.util.Base64

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
    init {
        require(version == 1) {
            "version must be 1"
        }
        require(direction == "incoming" || direction == "outgoing") {
            "direction must be incoming or outgoing"
        }
        require(transferId.isExactNonEmptyProtocolString()) {
            "transfer_id must be an exact non-empty string"
        }
        require(lineageId.isExactNonEmptyProtocolString()) {
            "lineage_id must be an exact non-empty string"
        }
        require(accountId.isExactNonEmptyProtocolString()) {
            "account_id must be an exact non-empty string"
        }
        require(deviceId.isExactNonEmptyProtocolString()) {
            "device_id must be an exact non-empty string"
        }
        require(offlinePublicKey.isExactNonEmptyProtocolString()) {
            "offline_public_key must be an exact non-empty string"
        }
        requireNonNegativeAmountString(preBalance, "pre_balance")
        requireNonNegativeAmountString(postBalance, "post_balance")
        requireNonNegativeAmountString(preLockedBalance, "pre_locked_balance")
        requireNonNegativeAmountString(postLockedBalance, "post_locked_balance")
        require(preStateHash.isLowerHex32()) {
            "pre_state_hash must be 32-byte lowercase hex"
        }
        require(postStateHash.isLowerHex32()) {
            "post_state_hash must be 32-byte lowercase hex"
        }
        require(localRevision >= 0) {
            "local_revision must be non-negative"
        }
        require(counterpartyLineageId.isExactNonEmptyProtocolString()) {
            "counterparty_lineage_id must be an exact non-empty string"
        }
        require(counterpartyAccountId.isExactNonEmptyProtocolString()) {
            "counterparty_account_id must be an exact non-empty string"
        }
        require(counterpartyDeviceId.isExactNonEmptyProtocolString()) {
            "counterparty_device_id must be an exact non-empty string"
        }
        require(counterpartyOfflinePublicKey.isExactNonEmptyProtocolString()) {
            "counterparty_offline_public_key must be an exact non-empty string"
        }
        requireNonNegativeAmountString(amount, "amount")
        requireOptionalExactNonEmptyProtocolString(sourcePayload, "source_payload")
        requireCanonicalSignatureBase64(senderSignatureBase64, "sender_signature_base64")
        require(createdAtMs >= 0) {
            "created_at_ms must be non-negative"
        }
    }

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

    private fun requireOptionalExactNonEmptyProtocolString(value: String?, field: String) {
        if (value != null) {
            require(value.isExactNonEmptyProtocolString()) {
                "$field must be an exact non-empty string"
            }
        }
    }

    private fun requireNonNegativeAmountString(value: String, field: String) {
        val canonical = OfflineCashCodec.canonicalAmountString(value)
        require(!canonical.startsWith("-")) {
            "$field must be a non-negative amount"
        }
    }

    private fun requireCanonicalSignatureBase64(value: String, field: String) {
        val signature = try {
            require(value.isNotEmpty() && value == value.trim()) { "$field must be canonical base64" }
            val decoded = Base64.getDecoder().decode(value)
            require(decoded.isNotEmpty() && Base64.getEncoder().encodeToString(decoded) == value) {
                "$field must be canonical base64"
            }
            decoded
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical base64", ex)
        }
        require(signature.size == 64) {
            "$field must be 64 bytes"
        }
    }

    private fun String.isLowerHex32(): Boolean =
        length == 64 && all { it in '0'..'9' || it in 'a'..'f' }

    private fun String.isExactNonEmptyProtocolString(): Boolean =
        isNotEmpty() && trim() == this
}
