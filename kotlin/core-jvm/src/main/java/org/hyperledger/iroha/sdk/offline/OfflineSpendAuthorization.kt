package org.hyperledger.iroha.sdk.offline

import java.util.Base64

/**
 * Signed spend-limit authorization issued by Torii for an offline lineage.
 *
 * `deviceId` and `offlinePublicKey` are absent from Torii's cash-wire shape (Torii
 * strips them on response and reconstitutes them from `deviceBinding` on request).
 * They are modeled here as nullable and omitted from `toJsonMap()` when absent, so
 * the same class round-trips the cash shape without carrying internal lineage fields.
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
    val issuerSignatureBase64: String,
) {
    init {
        require(authorizationId.isExactNonEmptyProtocolString()) {
            "authorization_id must be an exact non-empty string"
        }
        require(lineageId.isExactNonEmptyProtocolString()) {
            "lineage_id must be an exact non-empty string"
        }
        require(accountId.isExactNonEmptyProtocolString()) {
            "account_id must be an exact non-empty string"
        }
        requireOptionalExactNonEmptyProtocolString(deviceId, "device_id")
        requireOptionalExactNonEmptyProtocolString(offlinePublicKey, "offline_public_key")
        require(verdictId.isExactNonEmptyProtocolString()) {
            "verdict_id must be an exact non-empty string"
        }
        requireNonNegativeAmountString(maxBalance, "max_balance")
        requireNonNegativeAmountString(maxTxValue, "max_tx_value")
        require(issuedAtMs >= 0 && refreshAtMs >= issuedAtMs && expiresAtMs > issuedAtMs) {
            "authorization validity window must be increasing"
        }
        requireCanonicalSignatureBase64(issuerSignatureBase64, "issuer_signature_base64")
    }

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
        map["issuer_signature_base64"] = issuerSignatureBase64
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

    private fun String.isExactNonEmptyProtocolString(): Boolean =
        isNotEmpty() && trim() == this
}
