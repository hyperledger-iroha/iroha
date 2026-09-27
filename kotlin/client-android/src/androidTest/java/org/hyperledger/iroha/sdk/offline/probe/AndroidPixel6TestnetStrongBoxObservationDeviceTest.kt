// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.os.Build
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.DataOutputStream
import java.io.FileOutputStream
import java.security.MessageDigest
import java.security.Signature
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1OctetString
import org.bouncycastle.asn1.ASN1Primitive
import org.bouncycastle.asn1.ASN1Sequence
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Physical test of the raw experimental observation, not a monetary qualification test. */
@RunWith(AndroidJUnit4::class)
class AndroidPixel6TestnetStrongBoxObservationDeviceTest {
    @Test fun pixel6ProducesScopeBoundStrongBoxObservationWithoutHardwareOneUseClaim() {
        assertTrue("This probe is scoped to the connected Pixel 6",
            isPixel6HardwareV1(Build.MANUFACTURER, Build.DEVICE))
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val network = MessageDigest.getInstance("SHA-256")
            .digest("pixel6-experimental-test-network".toByteArray(Charsets.US_ASCII))
            .also { it[31] = (it[31].toInt() or 1).toByte() }
        val frame = Pixel6TestnetDiagnosticSelectionV1.open().create(network,
            "${context.packageName}\u0000physical-probe\u0000".toByteArray(Charsets.UTF_8))
        val release = frame.copyOfRange(59, 91)
        val lane = frame.copyOfRange(219, 251)
        val before = frame.copyOfRange(428, 444)
        val after = frame.copyOfRange(444, 460)
        fun collect(releaseId: ByteArray): Pixel6TestnetObservationResultV1 =
            AndroidPixel6TestnetStrongBoxObservationV1.collect(
                context, network, releaseId, frame, lane, before, after,
            )
        val evidence = collect(release) as Pixel6TestnetObservationResultV1.Evidence
        assertFalse(evidence.hardwareOneUseQualified)
        assertEquals(AndroidPixel6TestnetStrongBoxObservationV1.PROFILE, evidence.profile)
        assertArrayEquals(network, evidence.networkId())
        assertArrayEquals(release, evidence.releaseId())
        assertEquals(32, evidence.attestationNonce().size)
        val certificate = CertificateFactory.getInstance("X.509")
            .generateCertificate(evidence.certificateChain().first().inputStream()) as X509Certificate
        assertArrayEquals(evidence.publicKey(), uncompressedP256Sec1V1(certificate.publicKey))
        val extension = certificate.getExtensionValue("1.3.6.1.4.1.11129.2.1.17")
        requireNotNull(extension)
        val wrapper = ASN1OctetString.getInstance(ASN1Primitive.fromByteArray(extension))
        val fields = ASN1Sequence.getInstance(ASN1Primitive.fromByteArray(wrapper.octets))
        assertArrayEquals(evidence.attestationChallenge(),
            ASN1OctetString.getInstance(fields.getObjectAt(4)).octets)
        assertEquals("StrongBox attestation is required", 2,
            ASN1Enumerated.getInstance(fields.getObjectAt(1)).value.toInt())
        assertEquals("StrongBox KeyMint is required", 2,
            ASN1Enumerated.getInstance(fields.getObjectAt(3)).value.toInt())
        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(certificate.publicKey)
        verifier.update(evidence.signedMessage())
        assertTrue(verifier.verify(evidence.signatureDer()))
        retainPublicCertificateChainV1(context.noBackupFilesDir, evidence.certificateChain())
        val replay = collect(release) as Pixel6TestnetObservationResultV1.Evidence
        assertTrue(replay.recovered)
        assertArrayEquals(evidence.signatureDer(), replay.signatureDer())
        val conflictingRelease = ByteArray(32) { 9 }
        conflictingRelease.copyInto(frame, 59)
        assertTrue(collect(conflictingRelease) is Pixel6TestnetObservationResultV1.Frozen)
    }

    /** Diagnostic export: big-endian magic/count/length-prefixed leaf-first public DER only. */
    private fun retainPublicCertificateChainV1(directory: java.io.File, chain: List<ByteArray>) {
        require(chain.size in 1..8 && chain.all { it.size in 1..16_384 }) {
            "Pixel 6 public attestation chain is outside diagnostic bounds"
        }
        val file = java.io.File(directory, "kagemusha-pixel6-observation-chain-v1.bin")
        FileOutputStream(file).use { raw ->
            val output = DataOutputStream(raw)
            output.writeInt(0x4b474331) // KGC1
            output.writeInt(chain.size)
            chain.forEach { certificate ->
                output.writeInt(certificate.size)
                output.write(certificate)
            }
            output.flush()
            raw.fd.sync()
        }
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(file.readBytes()).joinToString("") { "%02x".format(it.toInt() and 0xff) }
        Log.i("IrohaKeyMintProbe", "public leaf-first chain=${file.absolutePath}; SHA-256=$digest")
    }
}
