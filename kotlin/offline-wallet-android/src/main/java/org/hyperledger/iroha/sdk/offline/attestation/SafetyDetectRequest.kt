package org.hyperledger.iroha.sdk.offline.attestation

import org.hyperledger.iroha.sdk.offline.OfflineVerdictMetadata

/** Immutable payload describing the context of a Safety Detect attestation attempt. */
class SafetyDetectRequest private constructor(
    val certificateIdHex: String,
    val appId: String,
    val nonceHex: String,
    val packageName: String,
    val signingDigestSha256: String,
    requiredEvaluations: List<String>?,
    private val _maxTokenAgeMs: Long?,
    val metadata: OfflineVerdictMetadata.SafetyDetectMetadata?,
) {
    private val _requiredEvaluations: List<String>? = requiredEvaluations?.toList()

    fun requiredEvaluations(): List<String> =
        _requiredEvaluations ?: metadata?.requiredEvaluations ?: emptyList()

    fun maxTokenAgeMs(): Long? = _maxTokenAgeMs ?: metadata?.maxTokenAgeMs

    class Builder {
        private var certificateIdHex: String? = null
        private var appId: String? = null
        private var nonceHex: String? = null
        private var packageName: String? = null
        private var signingDigestSha256: String? = null
        private var requiredEvaluations: List<String>? = null
        private var maxTokenAgeMs: Long? = null
        private var metadata: OfflineVerdictMetadata.SafetyDetectMetadata? = null

        fun setCertificateIdHex(v: String) = apply { certificateIdHex = v }
        fun setAppId(v: String) = apply { appId = v }
        fun setNonceHex(v: String) = apply { nonceHex = v }
        fun setPackageName(v: String) = apply { packageName = v }
        fun setSigningDigestSha256(v: String) = apply { signingDigestSha256 = v }
        fun setRequiredEvaluations(v: List<String>) = apply { requiredEvaluations = v }
        fun setMaxTokenAgeMs(v: Long?) = apply { maxTokenAgeMs = v }
        fun setMetadata(v: OfflineVerdictMetadata.SafetyDetectMetadata) = apply { metadata = v }

        fun build(): SafetyDetectRequest {
            requireNotNull(certificateIdHex) { "certificateIdHex" }
            requireNotNull(appId) { "appId" }
            requireNotNull(nonceHex) { "nonceHex" }
            requireNotNull(packageName) { "packageName" }
            requireNotNull(signingDigestSha256) { "signingDigestSha256" }
            return SafetyDetectRequest(
                certificateIdHex!!, appId!!, nonceHex!!, packageName!!, signingDigestSha256!!,
                requiredEvaluations, maxTokenAgeMs, metadata,
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder() = Builder()
    }
}
