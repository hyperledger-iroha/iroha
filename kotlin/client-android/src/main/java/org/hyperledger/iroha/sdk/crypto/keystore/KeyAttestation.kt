package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.cert.CertificateEncodingException
import java.security.cert.X509Certificate

/**
 * Captures attestation material associated with a generated key.
 *
 * The attestation chain can be surfaced to application code so it can be pinned or transmitted to
 * remote verifiers. Certificates are stored in their DER-encoded form to avoid depending on Android
 * `android.security.keystore` types.
 */
class KeyAttestation(
    @JvmField val alias: String,
    certificateChain: List<ByteArray>,
) {
    private val _certificateChain: List<ByteArray> = certificateChain.map { it.copyOf() }

    /**
     * Returns the attestation certificate chain in DER form, starting from the leaf certificate.
     * The list is a defensive copy.
     */
    fun certificateChain(): List<ByteArray> = _certificateChain.map { it.copyOf() }

    fun toBuilder(): Builder = Builder()
        .setAlias(alias)
        .setCertificateChain(_certificateChain)

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()
    }

    /**
     * Mutable builder retained for Java interop and the incremental `addCertificate` pattern.
     */
    class Builder {
        private var alias: String = ""
        private val certificateChain: MutableList<ByteArray> = mutableListOf()

        fun setAlias(alias: String): Builder = apply {
            this.alias = alias
        }

        fun addCertificate(certificateDer: ByteArray): Builder = apply {
            certificateChain.add(certificateDer.copyOf())
        }

        fun addCertificate(certificate: X509Certificate): Builder = apply {
            try {
                addCertificate(certificate.encoded)
            } catch (ex: CertificateEncodingException) {
                throw IllegalArgumentException("Failed to encode certificate", ex)
            }
        }

        fun setCertificateChain(certs: List<ByteArray>?): Builder = apply {
            certificateChain.clear()
            certs?.forEach { addCertificate(it) }
        }

        fun build(): KeyAttestation = KeyAttestation(alias, certificateChain)
    }
}
