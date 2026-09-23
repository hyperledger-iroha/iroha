// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.EnumSet
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class KagemushaHardwareAppProfileSchemaV1Test {
    @Test
    fun allSixClassesUseExactRustOrdinalsAndCapabilityMasks() {
        val expectedMasks = intArrayOf(0x0000_ffff, 0x0000_ffff, 0x0000_ffff,
            0x0000_ffff, 0x0007_0000, 0x000b_0000)
        KagemushaHardwarePlatformClassV1.values().forEachIndexed { ordinal, platform ->
            assertEquals(ordinal, platform.ordinal)
            assertEquals(expectedMasks[ordinal], platform.requiredCapabilityMask)
            val profile = profile(platform)
            val roundTrip = KagemushaNoritoV1.decodeHardwareProfileShapeExact(
                KagemushaNoritoV1.encodeHardwareProfileShape(profile))
            assertEquals(platform, roundTrip.platformClass)
            assertEquals(expectedMasks[ordinal], roundTrip.capabilityMask)
            assertArrayEquals(profile.appAttestationAuthorityPolicyDigest(),
                roundTrip.appAttestationAuthorityPolicyDigest())
            val preimage = KagemushaNoritoV1.hardwareProfileIdPreimageShape(profile)
            assertEquals(413, preimage.size)
            assertArrayEquals(u32Le(ordinal), preimage.copyOfRange(80, 84))
            assertArrayEquals(u32Le(expectedMasks[ordinal]), preimage.copyOfRange(325, 329))
            assertArrayEquals(profile.appAttestationAuthorityPolicyDigest(), preimage.copyOfRange(381, 413))
            assertArrayEquals(
                domainDigest("iroha:kagemusha:v1:hardware-profile", preimage),
                KagemushaNoritoV1.expectedHardwareProfileIdShape(profile),
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            profile(KagemushaHardwarePlatformClassV1.ANDROID_KEYMINT, 0x0000_ffff)
        }
    }

    @Test
    fun appPolicyClassByteAndCredentialBindingMatchRustFieldOrder() {
        val canonicalEd25519Key = rustPublicKeyGoldenArchive()
        val apple = KagemushaAppAttestationAuthorityPolicyV1(
            canonicalEd25519Key, KagemushaHardwarePlatformClassV1.APPLE_APP_ATTEST,
            digest(21), digest(22), 86_400L,
        )
        val android = KagemushaAppAttestationAuthorityPolicyV1(
            canonicalEd25519Key, KagemushaHardwarePlatformClassV1.ANDROID_KEYMINT,
            digest(21), digest(22), 86_400L,
        )
        val policyPreimage =
            "iroha:kagemusha:v1:app-attestation-authority-policy\u0000".toByteArray(StandardCharsets.US_ASCII) +
                u64Le(canonicalEd25519Key.size.toLong()) + canonicalEd25519Key +
                byteArrayOf(5) + digest(21) + digest(22) + u64Le(86_400L)
        assertArrayEquals(sha256(policyPreimage), android.canonicalDigestShape())
        org.junit.jupiter.api.Assertions.assertFalse(apple.canonicalDigestShape().contentEquals(
            android.canonicalDigestShape()))

        val binding = KagemushaAppDevicePolicyBindingV1(
            digest(21), digest(22), digest(23), digest(24), digest(25), digest(26),
        )
        val bindingPreimage = "iroha:kagemusha:v1:app-device-static-binding\u0000"
            .toByteArray(StandardCharsets.US_ASCII) +
            digest(21) + digest(22) + digest(23) + digest(24) + digest(25) + digest(26)
        assertArrayEquals(sha256(bindingPreimage), binding.canonicalDigestShape())

        val credential = credential(binding.canonicalDigestShape())
        val decoded = KagemushaNoritoV1.decodeHardwareCredentialShapeExact(
            KagemushaNoritoV1.encodeHardwareCredentialShape(credential))
        assertArrayEquals(binding.canonicalDigestShape(), decoded.appPolicyBindingDigest())
        val preimage = KagemushaNoritoV1.hardwareCredentialIdPreimageShape(credential)
        assertEquals(409, preimage.size)
        assertArrayEquals(credential.laneCommitment(), preimage.copyOfRange(185, 217))
        assertArrayEquals(binding.canonicalDigestShape(), preimage.copyOfRange(377, 409))
        assertArrayEquals(domainDigest("iroha:kagemusha:v1:hardware-credential-id", preimage),
            KagemushaNoritoV1.expectedHardwareCredentialIdShape(credential))
    }

    @Test
    fun ordinaryAppProfileCannotEnterCurrentOemMonetaryCoordinator() {
        val profile = profile(KagemushaHardwarePlatformClassV1.ANDROID_KEYMINT)
        val credential = credential(digest(31), profile.hardwareProfileId())
        val qualification = KagemushaHardwareQualificationV1(
            1, profile, credential, digest(32), digest(33), digest(34),
            EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java),
        )
        assertThrows(IllegalArgumentException::class.java) { qualification.requireProductionReady() }
    }

    private fun profile(
        platform: KagemushaHardwarePlatformClassV1,
        mask: Int = platform.requiredCapabilityMask,
    ): KagemushaHardwareProfileV1 = KagemushaHardwareProfileV1(
        1, 1, digest(1), digest(2), platform, digest(3), digest(4), digest(5),
        digest(6), digest(7), 1L, key(), mask, digest(8), 1L, 100_000L, digest(9),
    )

    private fun credential(
        binding: ByteArray,
        profileId: ByteArray = digest(1),
    ): KagemushaHardwareCredentialV1 = KagemushaHardwareCredentialV1(
        1, digest(10), NetworkId.fromBytes(digest(11)), profileId, digest(12),
        digest(13), 1L, digest(14), digest(15), 1L, key(), digest(16), 2L, 90_000L,
        binding, KagemushaDeviceSignatureV1(ByteArray(64).also { it[31] = 1; it[63] = 1 }),
    )

    private fun key(): KagemushaDevicePublicKeyV1 = KagemushaDevicePublicKeyV1(
        hex("046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"),
    )

    private fun rustPublicKeyGoldenArchive(): ByteArray = hex(
        "4e5254300000b6b01d0a3d2b9cfe06ff97af6ba0f622004a00000000000000ff3888681ae90906" +
            "022100000000000000010001ed01f601d701b5012c0170013201d0013a01ec0169016f0120016801" +
            "bd015301100115012801f301c701b60108011b01ff010501a10166012d017f01c20145",
    )

    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun digest(value: Int): ByteArray = ByteArray(32) { value.toByte() }
    private fun sha256(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)
    private fun u32Le(value: Int): ByteArray = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()
    private fun u64Le(value: Long): ByteArray = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
    private fun domainDigest(domain: String, transcript: ByteArray): ByteArray = sha256(
        domain.toByteArray(StandardCharsets.US_ASCII) + byteArrayOf(0) +
            u64Le(transcript.size.toLong()) + transcript,
    )
}
