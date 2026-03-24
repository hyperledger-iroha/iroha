package org.hyperledger.iroha.sdk.offline

/** Operator-signed offline wallet certificate payload. */
class OfflineWalletCertificate(
    val controller: String,
    val operator: String,
    val allowance: OfflineAllowanceCommitment,
    val spendPublicKey: String,
    attestationReport: ByteArray,
    val issuedAtMs: Long,
    val expiresAtMs: Long,
    val policy: OfflineWalletPolicy,
    val operatorSignatureHex: String,
    metadata: Map<String, Any>?,
    val verdictIdHex: String?,
    val attestationNonceHex: String?,
    val refreshAtMs: Long?,
) {
    private val _attestationReport: ByteArray = attestationReport.copyOf()
    val metadata: Map<String, Any> = if (metadata == null) emptyMap() else metadata.toMap()

    val attestationReport: ByteArray get() = _attestationReport.copyOf()

    fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["controller"] = controller
        map["operator"] = operator
        map["allowance"] = allowance.toJsonMap()
        map["spend_public_key"] = spendPublicKey
        map["attestation_report"] = OfflineAllowanceCommitment.encodeBytes(_attestationReport)
        map["issued_at_ms"] = issuedAtMs
        map["expires_at_ms"] = expiresAtMs
        map["policy"] = policy.toJsonMap()
        map["operator_signature"] = operatorSignatureHex
        map["metadata"] = metadata
        map["verdict_id"] =
            if (verdictIdHex == null) null
            else OfflineHashLiteral.normalize(verdictIdHex, "verdict_id")
        map["attestation_nonce"] =
            if (attestationNonceHex == null) null
            else OfflineHashLiteral.normalize(attestationNonceHex, "attestation_nonce")
        map["refresh_at_ms"] = refreshAtMs
        return map
    }
}
