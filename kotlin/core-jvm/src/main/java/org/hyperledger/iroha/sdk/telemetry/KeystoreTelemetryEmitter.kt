package org.hyperledger.iroha.sdk.telemetry

import java.security.cert.X509Certificate
import java.security.MessageDigest
import java.util.Locale
import org.hyperledger.iroha.sdk.crypto.KeyGenerationOutcome
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata

/** Emits Android keystore attestation telemetry events. */
class KeystoreTelemetryEmitter private constructor(
    private val sink: TelemetrySink?,
    private val redaction: Redaction?,
    private val deviceProfileProvider: DeviceProfileProvider,
) {
    /**
     * Records a successful attestation verification/generation.
     *
     * @param securityLevel label for the attestation security level (e.g. "software", "tee", "strongbox")
     * @param leafCertificate the leaf certificate from the attestation chain
     */
    fun recordResult(alias: String?, metadata: KeyProviderMetadata?, securityLevel: String?, leafCertificate: X509Certificate?) {
        if (sink == null || redaction == null || securityLevel == null) return
        val aliasLabel = aliasLabel(alias) ?: return
        val digest = leafCertificateDigest(leafCertificate)
        val brandBucket = deviceBrandBucket()
        sink.emitSignal(
            "android.keystore.attestation.result",
            mapOf(
                "alias_label" to aliasLabel,
                "security_level" to securityLevel.lowercase(Locale.ROOT),
                "attestation_digest" to digest,
                "device_brand_bucket" to brandBucket,
                "provider" to (metadata?.name ?: "unknown"),
            ),
        )
    }

    /** Records a key generation event, including the preference requested and the hardware route used. */
    fun recordKeyGeneration(
        alias: String?,
        preferenceLabel: String?,
        metadata: KeyProviderMetadata?,
        route: KeyGenerationOutcome.Route?,
        fallback: Boolean,
    ) {
        if (sink == null || redaction == null) return
        val aliasLabel = aliasLabel(alias) ?: return
        val routeLabel = route?.name?.lowercase(Locale.ROOT) ?: "unknown"
        sink.emitSignal(
            "android.keystore.keygen",
            mapOf(
                "alias_label" to aliasLabel,
                "preference" to (preferenceLabel?.lowercase(Locale.ROOT) ?: "unknown"),
                "route" to routeLabel,
                "fallback" to fallback,
                "provider" to (metadata?.name ?: "unknown"),
                "device_brand_bucket" to deviceBrandBucket(),
            ),
        )
    }

    /** Records a failure to validate Ed25519 SPKI outputs from a key provider. */
    fun recordKeyValidationFailure(
        alias: String?,
        preferenceLabel: String?,
        metadata: KeyProviderMetadata?,
        phase: String?,
        reason: String?,
        spkiLength: Int,
        expectedLength: Int,
        spkiPrefixHex: String?,
    ) {
        if (sink == null || redaction == null) return
        val aliasLabel = aliasLabel(alias) ?: return
        val phaseLabel = if (phase.isNullOrBlank()) "unknown" else phase
        val failureReason = if (reason.isNullOrBlank()) "unknown" else reason
        val prefix = if (spkiPrefixHex.isNullOrBlank()) "unknown" else spkiPrefixHex
        sink.emitSignal(
            "android.keystore.key_validation.failure",
            mapOf(
                "alias_label" to aliasLabel,
                "preference" to (preferenceLabel?.lowercase(Locale.ROOT) ?: "unknown"),
                "provider" to (metadata?.name ?: "unknown"),
                "phase" to phaseLabel,
                "reason" to failureReason,
                "spki_length" to spkiLength,
                "expected_spki_length" to expectedLength,
                "spki_prefix" to prefix,
                "device_brand_bucket" to deviceBrandBucket(),
            ),
        )
    }

    /** Records an attestation failure. */
    fun recordFailure(alias: String?, metadata: KeyProviderMetadata?, reason: String?) {
        if (sink == null || redaction == null) return
        val aliasLabel = aliasLabel(alias) ?: return
        val failureReason = if (reason.isNullOrBlank()) "unknown" else reason
        sink.emitSignal(
            "android.keystore.attestation.failure",
            mapOf(
                "alias_label" to aliasLabel,
                "failure_reason" to failureReason,
                "provider" to (metadata?.name ?: "unknown"),
            ),
        )
    }

    private fun aliasLabel(alias: String?): String? {
        if (alias.isNullOrBlank()) return null
        if (redaction == null || !redaction.enabled) return null
        return redaction.hashIdentifier(alias).orElse(null)
    }

    private fun deviceBrandBucket(): String =
        deviceProfileProvider.snapshot()
            .map { it.bucket }
            .orElse("unknown")

    companion object {
        private val NOOP = KeystoreTelemetryEmitter(null, Redaction.disabled(), DeviceProfileProvider.disabled())

        @JvmStatic
        fun noop(): KeystoreTelemetryEmitter = NOOP

        @JvmStatic
        fun from(
            options: TelemetryOptions?,
            sink: TelemetrySink?,
            provider: DeviceProfileProvider?,
        ): KeystoreTelemetryEmitter {
            if (options == null || sink == null || !options.enabled) return noop()
            val safeProvider = provider ?: DeviceProfileProvider.disabled()
            return KeystoreTelemetryEmitter(sink, options.redaction, safeProvider)
        }

        private fun leafCertificateDigest(cert: X509Certificate?): String = try {
            if (cert == null) "unknown"
            else {
                val digest = MessageDigest.getInstance("SHA-256")
                bytesToHex(digest.digest(cert.encoded))
            }
        } catch (_: Exception) {
            "error"
        }

        private fun bytesToHex(bytes: ByteArray): String =
            bytes.joinToString("") { "%02x".format(it) }
    }
}
