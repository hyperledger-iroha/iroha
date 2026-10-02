package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.security.AlgorithmParameters
import java.security.MessageDigest
import java.security.cert.X509Certificate
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import org.bouncycastle.asn1.ASN1OctetString
import org.bouncycastle.asn1.ASN1Primitive
import org.bouncycastle.asn1.ASN1Sequence
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1Set
import org.bouncycastle.asn1.ASN1TaggedObject
import org.bouncycastle.asn1.BERTags
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets

/** Original-evidence parsing only. PKIX, governed roots/revocation and issuer policy are separate. */
object AndroidKeyAttestationOriginalV1 {
    const val KEY_DESCRIPTION_OID = "1.3.6.1.4.1.11129.2.1.17"
    @JvmStatic fun certificate(chain: List<X509Certificate>): X509Certificate {
        require(chain.size in 2..8) { "Android attestation chain is outside bounds" }
        val selected = chain.asReversed().firstOrNull { it.getExtensionValue(KEY_DESCRIPTION_OID) != null }
            ?: throw AttestationVerificationException("Android attestation extension is missing")
        if (!MessageDigest.isEqual(selected.publicKey.encoded, chain.first().publicKey.encoded)) {
            throw AttestationVerificationException("Android attestation does not describe the alias-held leaf key")
        }
        return selected
    }
    @JvmStatic fun challenge(chain: List<X509Certificate>): ByteArray {
        val extension = certificate(chain).getExtensionValue(KEY_DESCRIPTION_OID)
        val description = ASN1Sequence.getInstance(ASN1Primitive.fromByteArray(
            ASN1OctetString.getInstance(ASN1Primitive.fromByteArray(extension)).octets))
        require(description.size() == 8) { "Unsupported original KeyDescription layout" }
        return ASN1OctetString.getInstance(description.getObjectAt(4)).octets.copyOf()
    }
    /**
     * Parse the original signed extension's persistent P-256 app-key metadata. This does not
     * authenticate roots, revocation, package/Play policy or issuer admission. Legacy API28–30
     * combines this exact leaf/challenge correlation with actual KeyInfo hardware custody.
     */
    @JvmStatic fun persistentAppHardwareSecurityLevel(chain: List<X509Certificate>, expectedChallenge: ByteArray): AttestationResult.SecurityLevel =
        parsePersistentAppDescriptionOriginal(chain, expectedChallenge).securityLevel

    private fun parsePersistentAppDescriptionOriginal(
        chain: List<X509Certificate>,
        expectedChallenge: ByteArray,
    ): PersistentAppDescriptionV1 {
        require(expectedChallenge.size == 32 && expectedChallenge.any { it != 0.toByte() })
        require(chain.size in 2..8 && chain.all { it.encoded.size in 1..16_384 })
        val extension = certificate(chain).getExtensionValue(KEY_DESCRIPTION_OID)
        val payload = ASN1OctetString.getInstance(ASN1Primitive.fromByteArray(extension)).octets
        val description = ASN1Sequence.getInstance(ASN1Primitive.fromByteArray(payload))
        require(payload.contentEquals(description.getEncoded("DER")) && description.size() == 8) {
            "Original KeyDescription is not the exact canonical layout"
        }
        fun integer(index: Int) = (description.getObjectAt(index) as? ASN1Integer)?.value
            ?: throw AttestationVerificationException("Malformed original KeyDescription version")
        require(integer(0) in setOf(1L, 2L, 3L, 4L, 100L, 200L, 300L, 400L, 500L).map(BigInteger::valueOf))
        require(integer(2) in setOf(2L, 3L, 4L, 41L, 100L, 200L, 300L, 400L, 500L).map(BigInteger::valueOf))
        val attestation = (description.getObjectAt(1) as? ASN1Enumerated)?.value
        val key = (description.getObjectAt(3) as? ASN1Enumerated)?.value
        require(attestation == key && key in setOf(BigInteger.ONE, BigInteger.valueOf(2))) {
            "Original app key lacks matching TEE or StrongBox levels"
        }
        require(MessageDigest.isEqual(ASN1OctetString.getInstance(description.getObjectAt(4)).octets, expectedChallenge)) {
            "Original persistent app-key challenge differs"
        }
        ASN1OctetString.getInstance(description.getObjectAt(5))
        fun tags(index: Int): Map<Int, ASN1TaggedObject> {
            val sequence = description.getObjectAt(index) as? ASN1Sequence
                ?: throw AttestationVerificationException("Malformed original app-key authorizations")
            require(sequence.size() <= 128)
            val parsed = linkedMapOf<Int, ASN1TaggedObject>(); var prior = 0
            for (item in sequence) {
                val tagged = item as? ASN1TaggedObject
                    ?: throw AttestationVerificationException("App-key authorization is not context-tagged")
                require(tagged.hasContextTag(tagged.tagNo) && tagged.tagNo > prior) {
                    "Duplicate or unsorted original app-key authorization"
                }
                prior = tagged.tagNo; parsed[tagged.tagNo] = tagged
            }
            return parsed
        }
        val software = tags(6); val hardware = tags(7)
        require(405 !in software && 405 !in hardware) { "Finite-use key cannot be a persistent app identity" }
        require(software.keys.intersect(setOf(1, 2, 3, 5, 10, 303, 702, 704)).isEmpty()) {
            "Original app-key hardware authorization is software-enforced"
        }
        fun exactInteger(tag: Int, expected: Long) {
            val value = hardware[tag]?.getBaseUniversal(true, BERTags.INTEGER) as? ASN1Integer
            require(value?.value == BigInteger.valueOf(expected)) { "Original app-key hardware integer differs" }
        }
        fun exactSet(tag: Int, expected: Long) {
            val value = hardware[tag]?.getBaseUniversal(true, BERTags.SET) as? ASN1Set
            require(value?.size() == 1 && (value.getObjectAt(0) as? ASN1Integer)?.value == BigInteger.valueOf(expected)) {
                "Original app-key hardware purpose or digest differs"
            }
        }
        exactSet(1, 2); exactInteger(2, 3); exactInteger(3, 256); exactSet(5, 4)
        exactInteger(10, 1); exactInteger(702, 0)
        val actualPublicKey = publicKeySec1(chain) // Enforce actual leaf-key equality and exact P-256 parameters.
        val level = if (key == BigInteger.ONE) AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT else AttestationResult.SecurityLevel.STRONG_BOX
        return PersistentAppDescriptionV1(software, hardware, level, actualPublicKey)
    }

    /**
     * Parse the exact single-app profile from original extension DATA. This method does not
     * perform PKIX or authenticate policy; the public verifier must verify its own chain first.
     * The older public persistent subset deliberately does not acquire this tag709 requirement.
     */
    internal fun persistentAppIdentityOriginals(
        chain: List<X509Certificate>,
        expectedChallenge: ByteArray,
        expectedPackageName: String,
        expectedVersionCode: BigInteger,
        expectedSigningIdentitySha256: ByteArray,
    ): PersistentAppIdentityOriginalV1 {
        val persistent = parsePersistentAppDescriptionOriginal(chain, expectedChallenge)
        val applicationIds = listOfNotNull(persistent.software[709], persistent.hardware[709])
        require(applicationIds.size == 1) { "Original app identity authorization is missing or duplicated" }
        val applicationId = applicationIds.single().getBaseUniversal(true, BERTags.OCTET_STRING) as? ASN1OctetString
            ?: throw AttestationVerificationException("Malformed original app identity authorization")
        val original = applicationId.octets
        val identity = ASN1Primitive.fromByteArray(original) as? ASN1Sequence
            ?: throw AttestationVerificationException("Malformed original app identity sequence")
        require(original.contentEquals(identity.getEncoded("DER")) && identity.size() == 2) {
            "Original app identity is not the exact canonical layout"
        }
        val packages = identity.getObjectAt(0) as? ASN1Set
            ?: throw AttestationVerificationException("Malformed original app package set")
        val signers = identity.getObjectAt(1) as? ASN1Set
            ?: throw AttestationVerificationException("Malformed original app signing set")
        require(packages.size() == 1 && signers.size() == 1) {
            "Original app identity is not the closed single-package/signer profile"
        }
        val packageInfo = packages.getObjectAt(0) as? ASN1Sequence
            ?: throw AttestationVerificationException("Malformed original app package info")
        require(packageInfo.size() == 2) { "Original app package info has extra or missing fields" }
        val packageOctets = (packageInfo.getObjectAt(0) as? ASN1OctetString)?.octets
            ?: throw AttestationVerificationException("Malformed original app package name")
        require(packageOctets.size in 1..128) { "Original app package is outside bounds" }
        val packageName = StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(packageOctets)).toString()
        require(packageName.contains('.') && packageName.split('.').all { part ->
            part.isNotEmpty() && part.all { it in 'A'..'Z' || it in 'a'..'z' || it in '0'..'9' || it == '_' }
        } && packageName == expectedPackageName) { "Original app package differs from installed policy" }
        val version = (packageInfo.getObjectAt(1) as? ASN1Integer)?.value
            ?: throw AttestationVerificationException("Malformed original app version")
        require(version.signum() > 0 && version.bitLength() <= 64 && version == expectedVersionCode) {
            "Original app version differs from installed policy"
        }
        val signer = (signers.getObjectAt(0) as? ASN1OctetString)?.octets
            ?: throw AttestationVerificationException("Malformed original app signing identity")
        require(expectedSigningIdentitySha256.size == 32 && expectedSigningIdentitySha256.any { it != 0.toByte() } &&
            signer.size == 32 && MessageDigest.isEqual(signer, expectedSigningIdentitySha256)) {
            "Original app signing identity differs from installed policy"
        }
        return PersistentAppIdentityOriginalV1(
            persistent.securityLevel, packageName, version, persistent.publicKeySec1(), signer,
        )
    }

    private class PersistentAppDescriptionV1(
        software: Map<Int, ASN1TaggedObject>,
        hardware: Map<Int, ASN1TaggedObject>,
        val securityLevel: AttestationResult.SecurityLevel,
        publicKeySec1: ByteArray,
    ) {
        val software: Map<Int, ASN1TaggedObject> = software.toMap()
        val hardware: Map<Int, ASN1TaggedObject> = hardware.toMap()
        private val publicKey = publicKeySec1.copyOf()
        fun publicKeySec1(): ByteArray = publicKey.copyOf()
    }

    /** Parsed DATA only; no chain, issuer or policy authority can be reconstructed from it. */
    internal class PersistentAppIdentityOriginalV1 internal constructor(
        val securityLevel: AttestationResult.SecurityLevel,
        val packageName: String,
        val versionCode: BigInteger,
        publicKeySec1: ByteArray,
        signingIdentitySha256: ByteArray,
    ) {
        private val publicKey = publicKeySec1.copyOf()
        private val signingIdentity = signingIdentitySha256.copyOf()
        fun publicKeySec1(): ByteArray = publicKey.copyOf()
        fun signingIdentitySha256(): ByteArray = signingIdentity.copyOf()
    }
    @JvmStatic fun publicKeySec1(chain: List<X509Certificate>): ByteArray {
        val key = certificate(chain).publicKey as? ECPublicKey
            ?: throw AttestationVerificationException("Android attested app key is not EC")
        val p256 = AlgorithmParameters.getInstance("EC").apply { init(ECGenParameterSpec("secp256r1")) }
            .getParameterSpec(ECParameterSpec::class.java)
        require(key.params.curve == p256.curve && key.params.generator == p256.generator &&
            key.params.order == p256.order && key.params.cofactor == p256.cofactor) { "Android app key is not P-256" }
        fun coordinate(value: java.math.BigInteger): ByteArray {
            require(value.signum() >= 0 && value.bitLength() <= 256)
            val encoded = value.toByteArray().let { if (it.size == 33 && it[0] == 0.toByte()) it.copyOfRange(1, 33) else it }
            require(encoded.size <= 32); return ByteArray(32 - encoded.size) + encoded
        }
        return byteArrayOf(4) + coordinate(key.w.affineX) + coordinate(key.w.affineY)
    }
}
