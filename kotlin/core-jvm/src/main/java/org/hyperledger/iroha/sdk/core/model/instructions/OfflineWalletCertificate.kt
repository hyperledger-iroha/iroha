package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/** Canonical allowance bundle attached to `RegisterOfflineAllowance`. */
class OfflineWalletCertificate(
    @JvmField val controller: String,
    @JvmField val operator: String,
    @JvmField val allowance: OfflineAllowance,
    @JvmField val spendPublicKey: String,
    @JvmField val attestationReportBase64: String,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val policy: OfflineWalletPolicy,
    @JvmField val operatorSignatureHex: String,
    metadata: Map<String, String> = emptyMap(),
    @JvmField val verdictIdHex: String? = null,
    @JvmField val attestationNonceHex: String? = null,
    @JvmField val refreshAtMs: Long? = null,
) {
    private val _metadata: Map<String, String> = metadata.toMap()

    val metadata: Map<String, String> get() = _metadata

    init {
        require(controller.isNotBlank()) { "controller must not be blank" }
        require(operator.isNotBlank()) { "operator must not be blank" }
        require(spendPublicKey.isNotBlank()) { "spendPublicKey must not be blank" }
        require(attestationReportBase64.isNotBlank()) { "attestationReportBase64 must not be blank" }
        require(issuedAtMs > 0L) { "issuedAtMs must be positive" }
        require(expiresAtMs > 0L) { "expiresAtMs must be positive" }
        require(operatorSignatureHex.isNotBlank()) { "operatorSignatureHex must not be blank" }
    }

    fun appendArguments(target: MutableMap<String, String>) {
        target["certificate.controller"] = controller
        target["certificate.operator"] = operator
        allowance.appendArguments(target)
        target["certificate.spend_public_key"] = spendPublicKey
        target["certificate.attestation_report_b64"] = attestationReportBase64
        target["certificate.issued_at_ms"] = issuedAtMs.toString()
        target["certificate.expires_at_ms"] = expiresAtMs.toString()
        policy.appendArguments(target)
        target["certificate.operator_signature_hex"] = operatorSignatureHex
        _metadata.forEach { (key, value) -> target["certificate.metadata.$key"] = value }
        if (!verdictIdHex.isNullOrBlank()) {
            target["certificate.verdict_id_hex"] = verdictIdHex
        }
        if (!attestationNonceHex.isNullOrBlank()) {
            target["certificate.attestation_nonce_hex"] = attestationNonceHex
        }
        if (refreshAtMs != null) {
            target["certificate.refresh_at_ms"] = refreshAtMs.toString()
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OfflineWalletCertificate) return false
        return controller == other.controller
            && operator == other.operator
            && allowance == other.allowance
            && spendPublicKey == other.spendPublicKey
            && attestationReportBase64 == other.attestationReportBase64
            && issuedAtMs == other.issuedAtMs
            && expiresAtMs == other.expiresAtMs
            && policy == other.policy
            && operatorSignatureHex == other.operatorSignatureHex
            && _metadata == other._metadata
            && verdictIdHex == other.verdictIdHex
            && attestationNonceHex == other.attestationNonceHex
            && refreshAtMs == other.refreshAtMs
    }

    override fun hashCode(): Int = listOf(
        controller, operator, allowance, spendPublicKey, attestationReportBase64,
        issuedAtMs, expiresAtMs, policy, operatorSignatureHex, _metadata,
        verdictIdHex, attestationNonceHex, refreshAtMs,
    ).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OfflineWalletCertificate {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("certificate.metadata.")) {
                        val metadataKey = key.removePrefix("certificate.metadata.")
                        if (metadataKey.isNotEmpty()) {
                            put(metadataKey, value)
                        }
                    }
                }
            }
            return OfflineWalletCertificate(
                controller = requireArgument(arguments, "certificate.controller"),
                operator = requireArgument(arguments, "certificate.operator"),
                allowance = OfflineAllowance.fromArguments(arguments),
                spendPublicKey = requireArgument(arguments, "certificate.spend_public_key"),
                attestationReportBase64 = requireArgument(arguments, "certificate.attestation_report_b64"),
                issuedAtMs = requireLong(arguments, "certificate.issued_at_ms"),
                expiresAtMs = requireLong(arguments, "certificate.expires_at_ms"),
                policy = OfflineWalletPolicy.fromArguments(arguments),
                operatorSignatureHex = requireArgument(arguments, "certificate.operator_signature_hex"),
                metadata = metadata,
                verdictIdHex = arguments["certificate.verdict_id_hex"],
                attestationNonceHex = arguments["certificate.attestation_nonce_hex"],
                refreshAtMs = arguments["certificate.refresh_at_ms"]?.let { requireLong(arguments, "certificate.refresh_at_ms") },
            )
        }

        @OptIn(ExperimentalEncodingApi::class)
        @JvmStatic
        fun requireBase64(value: String, fieldName: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$fieldName must not be blank" }
            val decoded = try {
                Base64.decode(trimmed)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$fieldName must be base64", ex)
            }
            require(decoded.isNotEmpty()) { "$fieldName must decode to non-empty bytes" }
            return trimmed
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val raw = requireArgument(arguments, key)
            return raw.toLongOrNull()
                ?: throw IllegalArgumentException("Instruction argument '$key' must be numeric: $raw")
        }
    }
}
