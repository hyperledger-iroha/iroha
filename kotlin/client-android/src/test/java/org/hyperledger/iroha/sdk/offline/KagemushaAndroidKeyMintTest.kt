// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.security.GeneralSecurityException
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.junit.jupiter.api.Test

/** Exact argument and signature-flow tests for the physical Android KeyMint profile. */
class KagemushaAndroidKeyMintTest {
    @Test
    fun `high-level registration derives and uses the exact pre-key challenge`() {
        val backend = FakeBackend()
        val keyMint = KagemushaAndroidKeyMint.withBackendForTests(backend)
        val accountId = AccountAddress.fromAccount(
            hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
            "ed25519",
        ).toI105(0x02f1)
        val parameters = KagemushaAndroidKeyMint.RegistrationParameters(
            deviceId = "physical-device-1",
            accountId = accountId,
            assetDefinitionId = null,
            androidPackageName = "org.hyperledger.iroha.pk3",
            androidSigningCertificateSha256 = canonicalHash(0x11),
            deviceAuthorityPublicKey = KagemushaDevicePublicKeyV2(
                uncompressed(backend.keyPair.public as ECPublicKey),
            ),
            recentBlockHeight = 42,
            recentBlockHash = canonicalHash(0x13),
            expiresAtMs = 2_000_000_000_000L,
        )

        val generated = keyMint.generateRegistration(
            "kagemusha-registration-1",
            parameters,
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
        )
        val registration = generated.registration()

        assertContentEquals(parameters.attestationChallenge(), backend.request!!.challenge())
        assertContentEquals(parameters.attestationChallenge(), registration.challengeHash)
        assertContentEquals(
            generated.material().assertionPublicKeySec1(),
            registration.assertionPublicKey,
        )
        assertContentEquals(generated.material().attestationReport(), registration.attestationReport)
        assertEquals(1, registration.assertionUsageCountLimit)
        assertTrue(registration.oneUse)
        assertEquals(generated.material().keyId(), registration.keyId)
    }

    @Test
    fun `generates exact single-use P256 profile and signs preparation bytes`() {
        val backend = FakeBackend()
        val keyMint = KagemushaAndroidKeyMint.withBackendForTests(backend)
        val challenge = canonicalHash(0x21)

        val material = keyMint.generateRegistrationMaterial(
            "kagemusha-operation-1",
            challenge,
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
        )

        val request = requireNotNull(backend.request)
        assertEquals(1, backend.generateCalls)
        assertEquals("kagemusha-operation-1", request.alias())
        assertEquals("EC", request.keyAlgorithm())
        assertEquals("secp256r1", request.curveName())
        assertEquals(KagemushaAndroidKeyMint.PURPOSES, request.purposes())
        assertEquals("SHA-256", request.digest())
        assertEquals(1, request.maxUsageCount())
        assertFalse(request.strongBoxRequired())
        assertContentEquals(challenge, request.challenge())
        assertEquals(65, material.assertionPublicKeySec1().size)
        assertEquals(64, material.keyId().length)
        assertEquals(1, material.certificateChainDer().size)
        assertContentEquals(
            byteArrayOf(0x81.toByte(), 0x43, 0x30, 0x01, 0x01),
            material.attestationReport(),
        )

        val signingBytes = ByteArray(237) { index -> (index * 17 + 3).toByte() }
        val signatureDer = keyMint.signPreparationForAuthorization(material, signingBytes)
        assertEquals("SHA256withECDSA", backend.signatureAlgorithm)
        assertContentEquals(signingBytes, backend.signedMessage)
        assertTrue(backend.deleted)
        assertTrue(material.isConsumed())
        assertEquals(64, KagemushaP256Codec.rawLowSFromStrictDer(signatureDer).size)

        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(backend.keyPair.public)
        verifier.update(signingBytes)
        assertTrue(verifier.verify(signatureDer))
        assertFailsWith<IllegalStateException> {
            keyMint.signPreparationForAuthorization(material, signingBytes)
        }
    }

    @Test
    fun `fails closed before generation without API31 hardware single-use`() {
        val oldApi = FakeBackend().apply { apiLevel = 30 }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(oldApi).generateRegistrationMaterial(
                "old-api",
                canonicalHash(0x31),
                KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
            )
        }
        assertEquals(0, oldApi.generateCalls)

        val softwareUsageLimit = FakeBackend().apply { hardwareSingleUse = false }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(softwareUsageLimit)
                .generateRegistrationMaterial(
                    "software-limit",
                    canonicalHash(0x41),
                    KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
                )
        }
        assertEquals(0, softwareUsageLimit.generateCalls)
    }

    @Test
    fun `StrongBox is explicit required and never downgrades`() {
        val unavailable = FakeBackend().apply { strongBox = false }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(unavailable)
                .generateRegistrationMaterial(
                    "strongbox-unavailable",
                    canonicalHash(0x51),
                    KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED,
                )
        }
        assertEquals(0, unavailable.generateCalls)

        val generationFailure = FakeBackend().apply {
            strongBox = true
            failGeneration = true
        }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(generationFailure)
                .generateRegistrationMaterial(
                    "strongbox-failure",
                    canonicalHash(0x61),
                    KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED,
                )
        }
        assertEquals(1, generationFailure.generateCalls)
        assertTrue(generationFailure.request!!.strongBoxRequired())
        assertEquals("strongbox-failure", generationFailure.deletedAlias)
    }

    @Test
    fun `rejects untruthful generated hardware projection`() {
        val software = FakeBackend().apply { generatedInsideHardware = false }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(software).generateRegistrationMaterial(
                "software-key",
                canonicalHash(0x71),
                KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
            )
        }
        assertEquals("software-key", software.deletedAlias)

        val wrongUsage = FakeBackend().apply { remainingUsageCount = 2 }
        assertFailsWith<GeneralSecurityException> {
            KagemushaAndroidKeyMint.withBackendForTests(wrongUsage)
                .generateRegistrationMaterial(
                    "wrong-usage",
                    canonicalHash(0x73),
                    KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
                )
        }
        assertEquals("wrong-usage", wrongUsage.deletedAlias)
    }

    @Test
    fun `challenge is canonical and defensively copied`() {
        val backend = FakeBackend()
        val keyMint = KagemushaAndroidKeyMint.withBackendForTests(backend)
        assertFailsWith<IllegalArgumentException> {
            keyMint.generateRegistrationMaterial(
                "short-challenge",
                ByteArray(31),
                KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            keyMint.generateRegistrationMaterial(
                "noncanonical-hash",
                ByteArray(32),
                KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
            )
        }

        val challenge = canonicalHash(0x7b)
        keyMint.generateRegistrationMaterial(
            "defensive-challenge",
            challenge,
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
        )
        challenge[0] = (challenge[0].toInt() xor 0x7f).toByte()
        assertEquals(0x7b.toByte(), backend.request!!.challenge()[0])
    }

    private class FakeBackend : KagemushaAndroidKeyMint.Backend {
        val keyPair: KeyPair = KeyPairGenerator.getInstance("EC").run {
            initialize(ECGenParameterSpec("secp256r1"))
            generateKeyPair()
        }
        var apiLevel: Int = 31
        var hardwareSingleUse: Boolean = true
        var strongBox: Boolean = false
        var failGeneration: Boolean = false
        var generatedInsideHardware: Boolean = true
        var remainingUsageCount: Int = 1
        var generateCalls: Int = 0
        var request: KagemushaAndroidKeyMint.GenerationRequest? = null
        var signatureAlgorithm: String? = null
        var signedMessage: ByteArray? = null
        var deleted: Boolean = false
        var deletedAlias: String? = null

        override fun apiLevel(): Int = apiLevel

        override fun supportsHardwareSingleUse(): Boolean = hardwareSingleUse

        override fun supportsStrongBox(): Boolean = strongBox

        override fun generate(
            request: KagemushaAndroidKeyMint.GenerationRequest,
        ): KagemushaAndroidKeyMint.GeneratedKey {
            generateCalls += 1
            this.request = request
            if (failGeneration) {
                delete(request.alias())
                throw GeneralSecurityException("injected StrongBox generation failure")
            }
            return KagemushaAndroidKeyMint.GeneratedKey(
                uncompressed(keyPair.public as ECPublicKey),
                listOf(byteArrayOf(0x30, 0x01, 0x01)),
                generatedInsideHardware,
                request.strongBoxRequired(),
                remainingUsageCount,
            )
        }

        override fun sign(alias: String, algorithm: String, message: ByteArray): ByteArray {
            signatureAlgorithm = algorithm
            signedMessage = message.copyOf()
            return Signature.getInstance(algorithm).run {
                initSign(keyPair.private)
                update(message)
                sign()
            }
        }

        override fun delete(alias: String) {
            deleted = true
            deletedAlias = alias
        }
    }

    companion object {
        private fun canonicalHash(marker: Int): ByteArray =
            ByteArray(32) { marker.toByte() }.also { it[31] = (it[31].toInt() or 1).toByte() }

        private fun uncompressed(publicKey: ECPublicKey): ByteArray =
            ByteArray(65).also { result ->
                result[0] = 0x04
                copyCoordinate(publicKey.w.affineX.toByteArray(), result, 1)
                copyCoordinate(publicKey.w.affineY.toByteArray(), result, 33)
            }

        private fun copyCoordinate(signed: ByteArray, destination: ByteArray, offset: Int) {
            val sourceOffset = if (signed.size == 33 && signed[0].toInt() == 0) 1 else 0
            val length = signed.size - sourceOffset
            System.arraycopy(signed, sourceOffset, destination, offset + 32 - length, length)
        }

        private fun hex(value: String): ByteArray =
            ByteArray(value.length / 2) { index ->
                value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
    }
}
