package org.hyperledger.iroha.sdk.offline.attestation

import org.hyperledger.iroha.sdk.offline.OfflineVerdictMetadata

/** Immutable payload describing the context of a Play Integrity attestation attempt. */
class PlayIntegrityRequest private constructor(
    val certificateIdHex: String,
    val nonceHex: String,
    val cloudProjectNumber: Long,
    val environment: String,
    val packageName: String,
    val signingDigestSha256: String,
    allowedAppVerdicts: List<String>?,
    allowedDeviceVerdicts: List<String>?,
    private val _maxTokenAgeMs: Long?,
    val metadata: OfflineVerdictMetadata.PlayIntegrityMetadata?,
) {
    val allowedAppVerdicts: List<String> = allowedAppVerdicts?.toList() ?: emptyList()
    val allowedDeviceVerdicts: List<String> = allowedDeviceVerdicts?.toList() ?: emptyList()

    fun maxTokenAgeMs(): Long? = _maxTokenAgeMs ?: metadata?.maxTokenAgeMs

    class Builder {
        private var certificateIdHex: String? = null
        private var nonceHex: String? = null
        private var cloudProjectNumber: Long = 0
        private var environment: String? = null
        private var packageName: String? = null
        private var signingDigestSha256: String? = null
        private var allowedAppVerdicts: List<String>? = null
        private var allowedDeviceVerdicts: List<String>? = null
        private var maxTokenAgeMs: Long? = null
        private var metadata: OfflineVerdictMetadata.PlayIntegrityMetadata? = null

        fun setCertificateIdHex(v: String) = apply { certificateIdHex = v }
        fun setNonceHex(v: String) = apply { nonceHex = v }
        fun setCloudProjectNumber(v: Long) = apply { cloudProjectNumber = v }
        fun setEnvironment(v: String) = apply { environment = v }
        fun setPackageName(v: String) = apply { packageName = v }
        fun setSigningDigestSha256(v: String) = apply { signingDigestSha256 = v }
        fun setAllowedAppVerdicts(v: List<String>) = apply { allowedAppVerdicts = v }
        fun setAllowedDeviceVerdicts(v: List<String>) = apply { allowedDeviceVerdicts = v }
        fun setMaxTokenAgeMs(v: Long?) = apply { maxTokenAgeMs = v }
        fun setMetadata(v: OfflineVerdictMetadata.PlayIntegrityMetadata) = apply { metadata = v }

        fun build(): PlayIntegrityRequest {
            requireNotNull(certificateIdHex) { "certificateIdHex" }
            requireNotNull(nonceHex) { "nonceHex" }
            requireNotNull(environment) { "environment" }
            requireNotNull(packageName) { "packageName" }
            requireNotNull(signingDigestSha256) { "signingDigestSha256" }
            require(cloudProjectNumber > 0) { "cloudProjectNumber must be positive" }
            return PlayIntegrityRequest(
                certificateIdHex!!, nonceHex!!, cloudProjectNumber, environment!!,
                packageName!!, signingDigestSha256!!, allowedAppVerdicts, allowedDeviceVerdicts,
                maxTokenAgeMs, metadata,
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder() = Builder()
    }
}
