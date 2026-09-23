// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.security.spec.ECPoint
import java.security.spec.EllipticCurve
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class AndroidKeyMintSingleUseProbeV1Test {
    @Test fun publicKeyUsesTheExactUncompressedSec1EncodingExpectedByCore() {
        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val publicKey = generator.generateKeyPair().public as ECPublicKey
        val sec1 = uncompressedP256Sec1V1(publicKey)
        assertEquals(65, sec1.size)
        assertEquals(0x04.toByte(), sec1[0])
        assertEquals(publicKey.w.affineX, BigInteger(1, sec1.copyOfRange(1, 33)))
        assertEquals(publicKey.w.affineY, BigInteger(1, sec1.copyOfRange(33, 65)))
    }

    @Test fun another256BitCurveCannotBeEncodedAsTheGovernedP256DeviceKey() {
        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val publicKey = generator.generateKeyPair().public as ECPublicKey
        val original = publicKey.params
        val substituted = object : ECPublicKey {
            override fun getW(): ECPoint = publicKey.w
            override fun getParams(): ECParameterSpec = ECParameterSpec(
                EllipticCurve(original.curve.field, original.curve.a.add(BigInteger.ONE),
                    original.curve.b),
                original.generator, original.order, original.cofactor,
            )
            override fun getAlgorithm(): String = "EC"
            override fun getFormat(): String = "X.509"
            override fun getEncoded(): ByteArray = publicKey.encoded
        }
        assertFailsWith<IllegalArgumentException> { uncompressedP256Sec1V1(substituted) }
    }

    @Test fun offCurvePointCannotBeEncodedAsTheGovernedP256DeviceKey() {
        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val publicKey = generator.generateKeyPair().public as ECPublicKey
        val substituted = object : ECPublicKey {
            override fun getW(): ECPoint = ECPoint(BigInteger.ONE, BigInteger.ONE)
            override fun getParams(): ECParameterSpec = publicKey.params
            override fun getAlgorithm(): String = "EC"
            override fun getFormat(): String = "X.509"
            override fun getEncoded(): ByteArray = publicKey.encoded
        }
        assertFailsWith<IllegalArgumentException> { uncompressedP256Sec1V1(substituted) }
    }

    private class FakeDevice : SingleUseProbeDeviceV1 {
        override var apiLevel = 31
        var feature = true
        var featureFails = false
        var challengeNumber = 1
        var generations = 0
        var deletions = 0
        var signingMessages = mutableListOf<ByteArray>()
        var secondSucceeds = false
        var generationFails = false
        var deleteFails = false
        override fun hasHardwareSingleUseFeature(): Boolean {
            if (featureFails) throw IllegalStateException("feature query failed")
            return feature
        }
        override fun newChallenge() = ByteArray(32) { challengeNumber.toByte() }.also {
            challengeNumber += 1
        }
        override fun hasAlias(alias: String): Boolean = false
        override fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1 {
            generations += 1
            if (generationFails) throw IllegalStateException("generated but chain unavailable")
            return ProbeKeyMaterialV1(byteArrayOf(1), listOf(byteArrayOf(2)))
        }
        override fun read(alias: String): ProbeKeyMaterialV1 =
            ProbeKeyMaterialV1(byteArrayOf(1), listOf(byteArrayOf(2)))
        override fun sign(alias: String, message: ByteArray): ByteArray {
            signingMessages.add(message.copyOf())
            if (signingMessages.size == 2 && !secondSucceeds) {
                throw IllegalStateException("key exhausted")
            }
            return byteArrayOf(signingMessages.size.toByte())
        }
        override fun delete(alias: String) {
            deletions += 1
            if (deleteFails) throw IllegalStateException("delete failed")
        }
    }

    @Test fun apiBelow31DoesNotCreateAKey() {
        val device = FakeDevice().apply { apiLevel = 30 }
        assertTrue(SingleUseProbeRunnerV1(device).run() is SingleUseProbeResultV1.Unavailable)
        assertEquals(0, device.generations)
    }

    @Test fun absentHardwareFeatureDoesNotCreateAKey() {
        val device = FakeDevice().apply { feature = false }
        assertTrue(SingleUseProbeRunnerV1(device).run() is SingleUseProbeResultV1.Unavailable)
        assertEquals(0, device.generations)
    }

    @Test fun failedHardwareFeatureQueryDoesNotCreateAKey() {
        val device = FakeDevice().apply { featureFails = true }
        val result = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Failed
        assertEquals("feature", result.stage)
        assertEquals(0, device.generations)
    }

    @Test fun freshChallengeAndDistinctAttemptsAreRecordedWithoutQualification() {
        val device = FakeDevice()
        val first = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Attempts
        assertTrue(first.first is SignAttemptV1.Signed)
        assertTrue(first.second is SignAttemptV1.Failed)
        assertFalse(device.signingMessages[0].contentEquals(device.signingMessages[1]))
        assertEquals(1, first.certificateChain.size)
        assertEquals(1, device.deletions)
        val next = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Attempts
        assertFalse(first.challenge.contentEquals(next.challenge))
    }

    @Test fun twoSuccessfulSignaturesRemainVisibleAsAnUnsafeObservation() {
        val device = FakeDevice().apply { secondSucceeds = true }
        val result = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Attempts
        assertTrue(result.first is SignAttemptV1.Signed)
        assertTrue(result.second is SignAttemptV1.Signed)
        assertEquals(1, device.deletions)
    }

    @Test fun generationFailureStillDeletesPossiblyCreatedAlias() {
        val device = FakeDevice().apply { generationFails = true }
        val result = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Failed
        assertEquals("generate", result.stage)
        assertEquals(1, device.deletions)
    }

    @Test fun cleanupFailureIsNeverHidden() {
        val device = FakeDevice().apply { deleteFails = true }
        val result = SingleUseProbeRunnerV1(device).run() as SingleUseProbeResultV1.Attempts
        assertEquals(IllegalStateException::class.java.name, result.cleanupExceptionClass)
    }
}
