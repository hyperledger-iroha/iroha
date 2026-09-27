// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.os.Build
import android.util.Log
import android.content.pm.PackageManager
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.MessageDigest
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.SecureRandom
import java.security.Signature
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.ASN1Null
import org.bouncycastle.asn1.ASN1OctetString
import org.bouncycastle.asn1.ASN1Primitive
import org.bouncycastle.asn1.ASN1Sequence
import org.bouncycastle.asn1.ASN1TaggedObject
import org.bouncycastle.asn1.BERTags

/** Physical, non-monetary KeyMint one-use diagnostic; run on each release device family. */
@RunWith(AndroidJUnit4::class)
class AndroidKeyMintSingleUseDeviceTest {
    /** Diagnostic only: a missing feature flag must not be mistaken for proof of hardware use. */
    @Test
    fun strongBoxOneUseWithoutFeatureFlagDiagnostic() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            Log.i("IrohaKeyMintProbe", "StrongBox no-flag diagnostic requires Android 12")
            return
        }
        val challenge = ByteArray(32).also(SecureRandom()::nextBytes)
        val alias = "iroha_keymint_strongbox_diagnostic_" +
            challenge.take(12).joinToString("") { "%02x".format(it.toInt() and 0xff) }
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        check(!store.containsAlias(alias)) { "diagnostic alias already exists" }
        try {
            val specification = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setAttestationChallenge(challenge)
                .setIsStrongBoxBacked(true)
                .setMaxUsageCount(1)
                .build()
            val generator = KeyPairGenerator.getInstance(
                KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore",
            )
            generator.initialize(specification)
            val pair = generator.generateKeyPair()
            val certificate = store.getCertificate(alias) as X509Certificate
            val attestation = attestationFacts(certificate)
            Log.i("IrohaKeyMintProbe", "StrongBox no-flag diagnostic: " +
                "attestation security=${attestation.attestationLevel}, " +
                "KeyMint security=${attestation.keyMintLevel}, " +
                "hardware tag 303=${attestation.hardwareRollback}, " +
                "hardware tag 405=${attestation.hardwareUsageLimit}, " +
                "software tag 303=${attestation.softwareRollback}, " +
                "software tag 405=${attestation.softwareUsageLimit}")
            assertArrayEquals(challenge, attestation.challenge)
            val message = MessageDigest.getInstance("SHA-256")
                .digest("nonmonetary StrongBox one-use diagnostic".toByteArray(Charsets.UTF_8))
            val signature = Signature.getInstance("SHA256withECDSA").run {
                initSign(pair.private)
                update(message)
                sign()
            }
            val valid = Signature.getInstance("SHA256withECDSA").run {
                initVerify(pair.public)
                update(message)
                verify(signature)
            }
            assertTrue("first StrongBox signature failed verification", valid)
            val second = runCatching {
                Signature.getInstance("SHA256withECDSA").run {
                    initSign(pair.private)
                    update(message)
                    sign()
                }
            }
            Log.i("IrohaKeyMintProbe", "StrongBox no-flag diagnostic: " +
                "first signature valid; second signed=${second.isSuccess}; " +
                "second failure=${second.exceptionOrNull()?.javaClass?.name}")
        } finally {
            if (store.containsAlias(alias)) store.deleteEntry(alias)
        }
    }

    @Test
    fun hardwareOneUseKeySignsExactlyOnceAndReturnsAttestation() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val packageManager = context.packageManager
        Log.i("IrohaKeyMintProbe", "${Build.MANUFACTURER} ${Build.MODEL} API ${Build.VERSION.SDK_INT}: " +
            "single-use=${packageManager.hasSystemFeature(PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY)}, " +
            "limited-use=${packageManager.hasSystemFeature(PackageManager.FEATURE_KEYSTORE_LIMITED_USE_KEY)}")
        val result = AndroidKeyMintSingleUseProbeV1.run(context)
        when (result) {
            is SingleUseProbeResultV1.Unavailable ->
                fail("${Build.MANUFACTURER} ${Build.MODEL}: ${result.reason}")
            is SingleUseProbeResultV1.Failed ->
                fail("${Build.MANUFACTURER} ${Build.MODEL}: ${result.stage} failed with " +
                    "${result.exceptionClass}; cleanup=${result.cleanupExceptionClass}")
            is SingleUseProbeResultV1.Attempts -> verifyAttempts(result)
        }
    }

    private fun verifyAttempts(result: SingleUseProbeResultV1.Attempts) {
        assertEquals(32, result.challenge.size)
        assertEquals(65, result.publicKey.size)
        assertEquals(0x04.toByte(), result.publicKey[0])
        assertFalse(result.certificateChain.isEmpty())
        val certificate = CertificateFactory.getInstance("X.509")
            .generateCertificate(result.certificateChain[0].inputStream()) as X509Certificate
        assertArrayEquals(result.publicKey, uncompressedP256Sec1V1(certificate.publicKey))
        val attestation = attestationFacts(certificate)
        Log.i("IrohaKeyMintProbe", "attestation security=${attestation.attestationLevel}, " +
            "KeyMint security=${attestation.keyMintLevel}, hardware tag 303=" +
            "${attestation.hardwareRollback}, hardware tag 405=${attestation.hardwareUsageLimit}, " +
            "software tag 303=${attestation.softwareRollback}, " +
            "software tag 405=${attestation.softwareUsageLimit}")
        assertArrayEquals(result.challenge, attestation.challenge)
        assertTrue("attestation security level is not hardware", attestation.attestationLevel in 1..2)
        assertTrue("KeyMint security level is not hardware", attestation.keyMintLevel in 1..2)
        assertTrue("rollback resistance is not hardware-enforced", attestation.hardwareRollback)
        assertEquals("one-use limit is not hardware-enforced", 1, attestation.hardwareUsageLimit)
        assertFalse("software claims rollback resistance", attestation.softwareRollback)
        assertEquals("software claims a usage limit", null, attestation.softwareUsageLimit)

        val first = result.first
        assertTrue("first signing attempt failed: $first", first is SignAttemptV1.Signed)
        val signature = (first as SignAttemptV1.Signed).signature
        val message = MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:keymint-one-use-probe:v1:first\u0000".toByteArray(Charsets.UTF_8) +
                result.challenge,
        )
        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(certificate.publicKey)
        verifier.update(message)
        assertTrue("first signature does not match the attested public key", verifier.verify(signature))
        assertTrue("the second signing attempt succeeded", result.second is SignAttemptV1.Failed)
        assertEquals(null, result.cleanupExceptionClass)

        // Public fingerprints identify the captured evidence without logging certificate bytes.
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(result.certificateChain[0])
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }
        Log.i("IrohaKeyMintProbe", "${Build.MANUFACTURER} ${Build.MODEL}: " +
            "one-use signature verified; second use rejected; leaf SHA-256=$digest")
    }

    private data class AttestationFacts(
        val challenge: ByteArray,
        val attestationLevel: Int,
        val keyMintLevel: Int,
        val hardwareRollback: Boolean,
        val hardwareUsageLimit: Int?,
        val softwareRollback: Boolean,
        val softwareUsageLimit: Int?,
    )

    private fun attestationFacts(certificate: X509Certificate): AttestationFacts {
        val extension = certificate.getExtensionValue("1.3.6.1.4.1.11129.2.1.17")
            ?: error("Android KeyMint attestation extension is absent")
        val wrapper = ASN1OctetString.getInstance(ASN1Primitive.fromByteArray(extension))
        val fields = ASN1Sequence.getInstance(ASN1Primitive.fromByteArray(wrapper.octets))
        require(fields.size() == 8) { "unexpected Android attestation layout" }
        val software = ASN1Sequence.getInstance(fields.getObjectAt(6))
        val hardware = ASN1Sequence.getInstance(fields.getObjectAt(7))
        return AttestationFacts(
            challenge = ASN1OctetString.getInstance(fields.getObjectAt(4)).octets,
            attestationLevel = ASN1Enumerated.getInstance(fields.getObjectAt(1)).value.toInt(),
            keyMintLevel = ASN1Enumerated.getInstance(fields.getObjectAt(3)).value.toInt(),
            hardwareRollback = tagged(hardware, 303)?.getBaseUniversal(true, BERTags.NULL) is ASN1Null,
            hardwareUsageLimit = taggedInteger(hardware, 405),
            softwareRollback = tagged(software, 303)?.getBaseUniversal(true, BERTags.NULL) is ASN1Null,
            softwareUsageLimit = taggedInteger(software, 405),
        )
    }

    private fun tagged(sequence: ASN1Sequence, number: Int): ASN1TaggedObject? =
        (0 until sequence.size())
            .map { ASN1TaggedObject.getInstance(sequence.getObjectAt(it)) }
            .firstOrNull { it.tagNo == number }

    private fun taggedInteger(sequence: ASN1Sequence, number: Int): Int? =
        (tagged(sequence, number)?.getBaseUniversal(true, BERTags.INTEGER) as? ASN1Integer)
            ?.value?.toInt()
}
