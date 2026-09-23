// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.MessageDigest
import java.security.SecureRandom
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
        assertEquals("This probe is scoped to the connected Pixel 6", "Pixel 6", Build.MODEL)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val network = MessageDigest.getInstance("SHA-256")
            .digest("pixel6-experimental-test-network".toByteArray(Charsets.US_ASCII))
        val release = MessageDigest.getInstance("SHA-256")
            .digest("pixel6-experimental-test-release".toByteArray(Charsets.US_ASCII))
        val lane = ByteArray(32).also(SecureRandom()::nextBytes)
        val before = ByteArray(16)
        val after = byteArrayOf(1) + ByteArray(15)
        val frameDomain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
            .toByteArray(Charsets.US_ASCII)
        val frame = ByteArray(460)
        frameDomain.copyInto(frame)
        frame[49] = 0x93.toByte()
        frame[50] = 1
        frame[57] = 1
        release.copyInto(frame, 59)
        frame[91] = 1
        frame[123] = 1
        frame[155] = 1
        network.copyInto(frame, 187)
        lane.copyInto(frame, 219)
        frame[251] = 1
        frame[283] = 1
        frame[291] = 1
        frame[323] = 1
        frame[331] = 1
        frame[332] = 1
        before.copyInto(frame, 428)
        after.copyInto(frame, 444)
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
        val replay = collect(release) as Pixel6TestnetObservationResultV1.Evidence
        assertTrue(replay.recovered)
        assertArrayEquals(evidence.signatureDer(), replay.signatureDer())
        val conflictingRelease = ByteArray(32) { 9 }
        conflictingRelease.copyInto(frame, 59)
        assertTrue(collect(conflictingRelease) is Pixel6TestnetObservationResultV1.Frozen)
    }
}
