package org.hyperledger.iroha.sdk.offline

import java.util.Base64

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
    init {
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
        require(assetDefinitionId.isExactNonEmptyProtocolString()) {
            "asset_definition_id must be an exact non-empty string"
        }
        requireNonNegativeAmountString(balance, "balance")
        requireNonNegativeAmountString(lockedBalance, "locked_balance")
        require(serverRevision >= 0) {
            "server_revision must be non-negative"
        }
        require(serverStateHash.isLowerHex32()) {
            "server_state_hash must be 32-byte lowercase hex"
        }
        require(pendingLocalRevision >= 0) {
            "pending_local_revision must be non-negative"
        }
        requireCanonicalSignatureBase64(issuerSignatureBase64, "issuer_signature_base64")
    }

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
