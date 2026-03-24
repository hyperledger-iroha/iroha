package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.io.ByteArrayInputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.security.cert.CertPathValidator
import java.security.cert.CertPathValidatorException
import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.PKIXParameters
import java.security.cert.TrustAnchor
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation

private const val ATTESTATION_OID = "1.3.6.1.4.1.11129.2.1.17"

/**
 * Validates Android key attestation certificate chains and extracts metadata required by higher
 * level policy checks.
 */
class AttestationVerifier private constructor(
    trustAnchors: Set<TrustAnchor>,
    private val requireStrongBox: Boolean,
) {
    private val trustAnchors: Set<TrustAnchor> = trustAnchors.toSet()

    /** Validates `attestation` against the configured policy. */
    @Throws(AttestationVerificationException::class)
    fun verify(attestation: KeyAttestation): AttestationResult = verify(attestation, null)

    /**
     * Validates `attestation` and checks that the embedded challenge matches `expectedChallenge`
     * when provided.
     */
    @Throws(AttestationVerificationException::class)
    fun verify(attestation: KeyAttestation, expectedChallenge: ByteArray?): AttestationResult {
        val chain = decodeChain(attestation)
        if (chain.isEmpty()) {
            throw AttestationVerificationException("Attestation certificate chain is empty")
        }
        val leaf = chain[0]

        validateCertificatePath(chain)

        val description = parseKeyDescription(leaf)
        if (expectedChallenge != null
            && !MessageDigest.isEqual(expectedChallenge, description.attestationChallenge)
        ) {
            throw AttestationVerificationException("Attestation challenge mismatch")
        }
        if (requireStrongBox
            && description.attestationSecurityLevel != AttestationResult.SecurityLevel.STRONG_BOX
        ) {
            throw AttestationVerificationException("StrongBox attestation required by policy")
        }

        return AttestationResult(
            alias = attestation.alias,
            certificateChain = chain,
            attestationSecurityLevel = description.attestationSecurityLevel,
            keymasterSecurityLevel = description.keymasterSecurityLevel,
            attestationChallenge = description.attestationChallenge,
            uniqueId = description.uniqueId,
            softwareAuthorisationsPresent = description.softwareAuthorisationsLength > 0,
            teeAuthorisationsPresent = description.teeAuthorisationsLength > 0,
            strongBoxAuthorisationsPresent = description.strongBoxAuthorisationsLength > 0,
        )
    }

    private fun decodeChain(attestation: KeyAttestation): List<X509Certificate> {
        val factory: CertificateFactory
        try {
            factory = CertificateFactory.getInstance("X.509")
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex)
        }

        return attestation.certificateChain().map { certificateDer ->
            try {
                factory.generateCertificate(ByteArrayInputStream(certificateDer)) as X509Certificate
            } catch (ex: CertificateException) {
                throw AttestationVerificationException("Failed to decode attestation certificate", ex)
            }
        }
    }

    private fun validateCertificatePath(chain: List<X509Certificate>) {
        val factory: CertificateFactory
        try {
            factory = CertificateFactory.getInstance("X.509")
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex)
        }

        val certPath = try {
            factory.generateCertPath(chain)
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Failed to construct attestation CertPath", ex)
        }

        val validator = try {
            CertPathValidator.getInstance("PKIX")
        } catch (ex: Exception) {
            throw AttestationVerificationException("Unable to acquire PKIX CertPathValidator", ex)
        }

        val parameters = try {
            PKIXParameters(trustAnchors)
        } catch (ex: Exception) {
            throw AttestationVerificationException("Invalid PKIX parameters", ex)
        }
        parameters.isRevocationEnabled = false

        try {
            validator.validate(certPath, parameters)
        } catch (ex: CertPathValidatorException) {
            throw AttestationVerificationException(
                "Attestation certificate path validation failed", ex
            )
        } catch (ex: Exception) {
            throw AttestationVerificationException(
                "Unexpected failure validating attestation certificate path", ex
            )
        }
    }

    private fun parseKeyDescription(leaf: X509Certificate): KeyDescription {
        val extension = leaf.getExtensionValue(ATTESTATION_OID)
            ?: throw AttestationVerificationException(
                "Leaf certificate does not contain Android attestation extension"
            )

        val outer = DerReader(extension)
        val octetString = outer.readOctetString()
        if (outer.hasRemaining()) {
            throw AttestationVerificationException("Unexpected data after attestation extension")
        }

        val reader = DerReader.sequence(octetString)
        val attestationVersion = reader.readInteger()
        if (attestationVersion <= 0) {
            throw AttestationVerificationException(
                "Invalid attestation version: $attestationVersion"
            )
        }

        val attestationLevel =
            AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated())
        val keymasterVersion = reader.readInteger()
        if (keymasterVersion < 0) {
            throw AttestationVerificationException(
                "Invalid keymaster version: $keymasterVersion"
            )
        }
        val keymasterLevel =
            AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated())
        val challenge = reader.readOctetString()
        val uniqueId = reader.readOctetString()
        val softwareEnforced = reader.readSequenceBytes()
        val teeEnforced = reader.readSequenceBytes()
        var strongBoxEnforced = ByteArray(0)
        if (reader.hasRemaining()) {
            strongBoxEnforced = reader.readSequenceBytes()
        }
        if (reader.hasRemaining()) {
            throw AttestationVerificationException("Unexpected trailing data in attestation")
        }

        return KeyDescription(
            attestationSecurityLevel = attestationLevel,
            keymasterSecurityLevel = keymasterLevel,
            attestationChallenge = challenge,
            uniqueId = uniqueId,
            softwareAuthorisationsLength = softwareEnforced.size,
            teeAuthorisationsLength = teeEnforced.size,
            strongBoxAuthorisationsLength = strongBoxEnforced.size,
        )
    }

    /** Builder used to configure `AttestationVerifier` instances. */
    class Builder internal constructor() {
        private val trustedRoots = linkedSetOf<X509Certificate>()
        private var requireStrongBox = false

        /** Adds a trusted root certificate in DER form. */
        @Throws(AttestationVerificationException::class)
        fun addTrustedRoot(certificateDer: ByteArray): Builder = apply {
            try {
                val factory = CertificateFactory.getInstance("X.509")
                trustedRoots.add(
                    factory.generateCertificate(ByteArrayInputStream(certificateDer)) as X509Certificate
                )
            } catch (ex: CertificateException) {
                throw AttestationVerificationException("Failed to decode trusted root certificate", ex)
            }
        }

        /** Adds a trusted root certificate. */
        fun addTrustedRoot(certificate: X509Certificate): Builder = apply {
            trustedRoots.add(certificate)
        }

        /** Requires StrongBox-backed attestation when `enabled` is `true`. */
        fun requireStrongBox(enabled: Boolean): Builder = apply {
            this.requireStrongBox = enabled
        }

        fun build(): AttestationVerifier {
            check(trustedRoots.isNotEmpty()) {
                "At least one trusted root certificate is required"
            }
            val anchors = trustedRoots.mapTo(linkedSetOf()) { TrustAnchor(it, null) }
            return AttestationVerifier(anchors, requireStrongBox)
        }
    }

    private class KeyDescription(
        val attestationSecurityLevel: AttestationResult.SecurityLevel,
        val keymasterSecurityLevel: AttestationResult.SecurityLevel,
        val attestationChallenge: ByteArray,
        val uniqueId: ByteArray,
        val softwareAuthorisationsLength: Int,
        val teeAuthorisationsLength: Int,
        val strongBoxAuthorisationsLength: Int,
    )

    private class DerReader(private val buffer: ByteArray) {
        private var offset = 0

        fun hasRemaining(): Boolean = offset < buffer.size

        fun readInteger(): Int = readIntegerWithTag(TAG_INTEGER)

        fun readEnumerated(): Int = readIntegerWithTag(TAG_ENUMERATED)

        fun readOctetString(): ByteArray = readWithExpectedTag(TAG_OCTET_STRING)

        fun readSequenceBytes(): ByteArray = readWithExpectedTag(TAG_SEQUENCE)

        private fun readIntegerWithTag(expectedTag: Int): Int {
            val value = readWithExpectedTag(expectedTag)
            try {
                return BigInteger(value).intValueExact()
            } catch (ex: ArithmeticException) {
                throw AttestationVerificationException("Integer value out of range", ex)
            }
        }

        private fun readWithExpectedTag(expectedTag: Int): ByteArray {
            val tag = readTag()
            if (tag != expectedTag) {
                throw AttestationVerificationException(
                    "Unexpected DER tag. expected=0x%02X actual=0x%02X".format(expectedTag, tag)
                )
            }
            val length = readLength()
            if (length < 0) {
                throw AttestationVerificationException("Invalid DER length")
            }
            if (offset + length > buffer.size) {
                throw AttestationVerificationException("DER value overruns buffer")
            }
            val value = buffer.copyOfRange(offset, offset + length)
            offset += length
            return value
        }

        private fun readTag(): Int {
            if (offset >= buffer.size) {
                throw AttestationVerificationException("Unexpected end of DER input")
            }
            return buffer[offset++].toInt() and 0xFF
        }

        private fun readLength(): Int {
            if (offset >= buffer.size) {
                throw AttestationVerificationException("Unexpected end of DER input")
            }
            val lengthByte = buffer[offset++].toInt() and 0xFF
            if (lengthByte and 0x80 == 0) return lengthByte
            val lengthOctets = lengthByte and 0x7F
            if (lengthOctets == 0 || lengthOctets > 4) {
                throw AttestationVerificationException("Unsupported DER length encoding")
            }
            var length = 0
            for (i in 0 until lengthOctets) {
                if (offset >= buffer.size) {
                    throw AttestationVerificationException("Invalid DER length encoding")
                }
                length = (length shl 8) or (buffer[offset++].toInt() and 0xFF)
            }
            return length
        }

        companion object {
            private const val TAG_SEQUENCE = 0x30
            private const val TAG_INTEGER = 0x02
            private const val TAG_ENUMERATED = 0x0A
            private const val TAG_OCTET_STRING = 0x04

            fun sequence(data: ByteArray): DerReader {
                val reader = DerReader(data)
                return DerReader(reader.readWithExpectedTag(TAG_SEQUENCE))
            }
        }
    }

    companion object {
        /** Creates a verifier that trusts the supplied root certificates. */
        @JvmStatic
        fun builder(): Builder = Builder()
    }
}
