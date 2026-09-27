// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.UserManager
import android.util.Log
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest
import java.security.KeyPairGenerator
import java.security.KeyFactory
import java.security.KeyStore
import java.security.PrivateKey
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
    private val restartAlias = "iroha_keymint_pixel6_restart_diagnostic_v1"
    private val restartMarker = "iroha_keymint_pixel6_restart_diagnostic_v1.marker"

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

    /** Stage 1: provision an unused StrongBox key, then end this instrumentation process. */
    @Test
    fun pixel6RestartStage1ProvisionUnusedKey() {
        requirePixel6RestartProbe()
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        check(!store.containsAlias(restartAlias)) { "restart diagnostic alias already exists" }
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val marker = File(context.noBackupFilesDir, restartMarker)
        check(!marker.exists()) { "restart diagnostic marker already exists" }
        val challenge = MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:pixel6-restart-diagnostic:v1".toByteArray(Charsets.US_ASCII),
        )
        val specification = KeyGenParameterSpec.Builder(restartAlias, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setAttestationChallenge(challenge)
            .setIsStrongBoxBacked(true)
            .setMaxUsageCount(1)
            .build()
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
        generator.initialize(specification)
        generator.generateKeyPair()
        val certificate = store.getCertificate(restartAlias) as X509Certificate
        val facts = attestationFacts(certificate)
        assertArrayEquals(challenge, facts.challenge)
        assertEquals(2, facts.attestationLevel)
        assertEquals(2, facts.keyMintLevel)
        assertFalse(facts.hardwareRollback)
        assertEquals(null, facts.hardwareUsageLimit)
        assertEquals(1, facts.softwareUsageLimit)
        Log.i("IrohaKeyMintProbe", "restart stage 1: unused alias provisioned; " +
            "hardware303=${facts.hardwareRollback}, hardware405=${facts.hardwareUsageLimit}, " +
            "software303=${facts.softwareRollback}, software405=${facts.softwareUsageLimit}; " +
            "boot=${diagnosticBootId()}")
    }

    /** Stage 2: a new app process performs the first signature and records only public evidence. */
    @Test
    fun pixel6RestartStage2SignOnceInNewProcess() {
        requirePixel6RestartProbe()
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val key = store.getKey(restartAlias, null) as? PrivateKey
            ?: error("restart diagnostic key was not retained across app restart")
        val certificate = store.getCertificate(restartAlias) as X509Certificate
        val message = MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:pixel6-restart-first-signature:v1".toByteArray(Charsets.US_ASCII),
        )
        val signature = Signature.getInstance("SHA256withECDSA").run {
            initSign(key)
            update(message)
            sign()
        }
        assertTrue(Signature.getInstance("SHA256withECDSA").run {
            initVerify(certificate.publicKey)
            update(message)
            verify(signature)
        })
        val signatureDigest = MessageDigest.getInstance("SHA-256").digest(signature)
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }
        val marker = File(InstrumentationRegistry.getInstrumentation().targetContext.noBackupFilesDir,
            restartMarker)
        FileOutputStream(marker).use { output ->
            output.write((diagnosticBootId() + "\n" + signatureDigest + "\n")
                .toByteArray(Charsets.US_ASCII))
            output.fd.sync()
        }
        Log.i("IrohaKeyMintProbe", "restart stage 2: first signature verified; " +
            "signature SHA-256=$signatureDigest; alias remains=${store.containsAlias(restartAlias)}")
    }

    /** Stage 3: after a device reboot, the consumed alias cannot produce a second signature. */
    @Test
    fun pixel6RestartStage3RejectSecondUseAfterReboot() {
        requirePixel6RestartProbe()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val userManager = context.getSystemService(Context.USER_SERVICE) as UserManager
        assertTrue("unlock the Pixel 6 with its PIN after reboot before testing one-use persistence",
            userManager.isUserUnlocked)
        val marker = File(InstrumentationRegistry.getInstrumentation().targetContext.noBackupFilesDir,
            restartMarker)
        val recorded = marker.readLines(Charsets.US_ASCII)
        check(recorded.size == 2 && recorded[1].matches(Regex("[0-9a-f]{64}"))) {
            "first-use diagnostic marker is absent or malformed"
        }
        check(recorded[0] != diagnosticBootId()) { "device was not rebooted after first use" }
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        // setMaxUsageCount documents permanent invalidation followed by deletion:
        // https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder#setMaxUsageCount(int)
        // A generic provider failure is not either documented exhausted-key observation.
        val observation = completeKeyMintRestartDiagnosticV1(
            verifyFreshControl = { verifyFreshRestartControlKey(store) },
            readConsumedKey = {
                if (!store.containsAlias(restartAlias)) null
                else store.getKey(restartAlias, null) as? PrivateKey
                    ?: error("consumed alias exists but has no private key")
            },
            signConsumedKey = { key ->
                Signature.getInstance("SHA256withECDSA").run {
                    initSign(key)
                    update("nonmonetary second-use diagnostic".toByteArray(Charsets.US_ASCII))
                    sign()
                }
            },
            isPermanentlyInvalidated = { it is KeyPermanentlyInvalidatedException },
            removeCompletedMarker = {
                check(marker.delete()) { "cannot remove restart diagnostic marker" }
            },
        )
        // Leave a still-present consumed alias untouched for inspection. Only the fresh
        // control alias is removed by this stage; unexpected errors also retain the marker.
        Log.i("IrohaKeyMintProbe", "restart stage 3: fresh StrongBox control verified; " +
            "consumed observation=$observation; boot=${diagnosticBootId()}")
    }

    /** A functioning fresh StrongBox key rules out a provider outage as the probe's success. */
    private fun verifyFreshRestartControlKey(store: KeyStore) {
        val nonce = ByteArray(32).also(SecureRandom()::nextBytes)
        val alias = "iroha_keymint_restart_control_" +
            nonce.joinToString("") { "%02x".format(it.toInt() and 0xff) }
        check(!store.containsAlias(alias)) { "restart control alias already exists" }
        try {
            val specification = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setIsStrongBoxBacked(true)
                .build()
            val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC,
                "AndroidKeyStore")
            generator.initialize(specification)
            val pair = generator.generateKeyPair()
            val info = KeyFactory.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
                .getKeySpec(pair.private, KeyInfo::class.java)
            assertEquals("restart control key must remain StrongBox backed",
                KeyProperties.SECURITY_LEVEL_STRONGBOX, info.securityLevel)
            val message = MessageDigest.getInstance("SHA-256").digest(
                "iroha:kagemusha:restart-control:v1".toByteArray(Charsets.US_ASCII) + nonce,
            )
            val signature = Signature.getInstance("SHA256withECDSA").run {
                initSign(pair.private)
                update(message)
                sign()
            }
            assertTrue("fresh StrongBox control signature failed verification",
                Signature.getInstance("SHA256withECDSA").run {
                    initVerify(pair.public)
                    update(message)
                    verify(signature)
                })
        } finally {
            if (store.containsAlias(alias)) store.deleteEntry(alias)
            check(!store.containsAlias(alias)) { "restart control alias cleanup failed" }
        }
    }

    private fun requirePixel6RestartProbe() {
        assertTrue("restart probe requires the physical Pixel 6",
            isPixel6HardwareV1(Build.MANUFACTURER, Build.DEVICE))
        assertTrue("restart probe requires Android 12 or later", Build.VERSION.SDK_INT >= 31)
    }

    private fun diagnosticBootId(): String = File("/proc/sys/kernel/random/boot_id")
        .readText(Charsets.US_ASCII).trim().also { check(it.matches(Regex("[0-9a-f-]{36}"))) }

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
