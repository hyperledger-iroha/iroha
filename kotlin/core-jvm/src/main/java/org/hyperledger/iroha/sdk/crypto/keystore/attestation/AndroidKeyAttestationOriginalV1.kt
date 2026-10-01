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
