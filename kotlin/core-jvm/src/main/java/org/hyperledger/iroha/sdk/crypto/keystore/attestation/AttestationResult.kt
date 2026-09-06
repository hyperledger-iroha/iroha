package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.security.cert.X509Certificate

/** Result of a successfully verified Android key attestation bundle. */
class AttestationResult(
    /** Alias associated with the generated key. */
    @JvmField val alias: String,
    certificateChain: List<X509Certificate>,
    /** Security level encoded in the attestation extension. */
    @JvmField val attestationSecurityLevel: SecurityLevel,
    /** Keymaster security level reported in the attestation extension. */
    @JvmField val keymasterSecurityLevel: SecurityLevel,
    attestationChallenge: ByteArray?,
    uniqueId: ByteArray?,
    /** Returns `true` when software-enforced authorisations were present. */
    @JvmField val softwareAuthorisationsPresent: Boolean,
    /** Returns `true` when TEE-enforced authorisations were present. */
    @JvmField val teeAuthorisationsPresent: Boolean,
    /** Returns `true` when StrongBox-enforced authorisations were present. */
    @JvmField val strongBoxAuthorisationsPresent: Boolean,
) {
    private val _certificateChain: List<X509Certificate> = certificateChain.toList()
    private val _attestationChallenge: ByteArray = attestationChallenge?.copyOf() ?: ByteArray(0)
    private val _uniqueId: ByteArray = uniqueId?.copyOf() ?: ByteArray(0)

    /** Leaf certificate that carries the attestation extension. */
    @JvmField
    val leafCertificate: X509Certificate

    init {
        require(_certificateChain.isNotEmpty()) { "certificateChain must not be empty" }
        leafCertificate = _certificateChain[0]
    }

    /** Immutable copy of the attestation certificate chain (leaf first). */
    fun certificateChain(): List<X509Certificate> = _certificateChain.toList()

    /** Challenge embedded in the attestation extension (copy). */
    fun attestationChallenge(): ByteArray = _attestationChallenge.copyOf()

    /** Unique identifier reported in the attestation extension (copy). */
    fun uniqueId(): ByteArray = _uniqueId.copyOf()

    /** Convenience helper indicating StrongBox-backed attestation. */
    val isStrongBoxAttestation: Boolean
        get() = attestationSecurityLevel == SecurityLevel.STRONG_BOX

    /** Enumeration of Android key attestation security levels. */
    enum class SecurityLevel(val encodedValue: Int) {
        SOFTWARE(0),
        TRUSTED_ENVIRONMENT(1),
        STRONG_BOX(2);

        companion object {
            @JvmStatic
            fun fromEncoded(value: Int): SecurityLevel =
                entries.find { it.encodedValue == value }
                    ?: throw IllegalArgumentException("Unknown security level value: $value")
        }
    }
}
