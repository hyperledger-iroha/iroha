package org.hyperledger.iroha.sdk.crypto

/**
 * Describes capabilities exposed by a key provider.
 *
 * The metadata is intentionally generic so it can be computed on desktop JVMs and populated with
 * richer details (attestation/StrongBox) when running on Android. Android-specific providers will
 * extend this to include platform attestation blobs in follow-up revisions.
 */
class KeyProviderMetadata(
    @JvmField val name: String = "unknown-provider",
    @JvmField val hardwareBacked: Boolean = false,
    @JvmField val strongBoxBacked: Boolean = false,
    @JvmField val secureElementBacked: Boolean = false,
    @JvmField val supportsAttestationCertificates: Boolean = false,
    @JvmField val securityLevel: HardwareSecurityLevel = HardwareSecurityLevel.NONE,
) {
    /** Indicates the strongest hardware security level available for generated keys. */
    enum class HardwareSecurityLevel {
        NONE,
        TRUSTED_ENVIRONMENT,
        STRONGBOX,
        SECURE_ELEMENT,
    }

    companion object {
        /** Creates metadata for a purely software provider. */
        @JvmStatic
        fun software(name: String): KeyProviderMetadata =
            KeyProviderMetadata(name = name)

        /** Creates metadata for a Trusted Environment backed provider (TEE). */
        @JvmStatic
        fun trustedEnvironment(name: String): KeyProviderMetadata =
            KeyProviderMetadata(
                name = name,
                hardwareBacked = true,
                securityLevel = HardwareSecurityLevel.TRUSTED_ENVIRONMENT,
            )

        /** Creates metadata for a StrongBox backed provider. */
        @JvmStatic
        fun strongBox(name: String, supportsAttestation: Boolean): KeyProviderMetadata =
            KeyProviderMetadata(
                name = name,
                hardwareBacked = true,
                strongBoxBacked = true,
                supportsAttestationCertificates = supportsAttestation,
                securityLevel = HardwareSecurityLevel.STRONGBOX,
            )

        /** Creates metadata for a discrete secure element backed provider. */
        @JvmStatic
        fun secureElement(name: String, supportsAttestation: Boolean): KeyProviderMetadata =
            KeyProviderMetadata(
                name = name,
                hardwareBacked = true,
                secureElementBacked = true,
                supportsAttestationCertificates = supportsAttestation,
                securityLevel = HardwareSecurityLevel.SECURE_ELEMENT,
            )
    }
}
