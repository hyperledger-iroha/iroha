// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.pm.ApplicationInfo
import android.security.keystore.KeyProperties
import java.io.File
import java.security.InvalidKeyException
import java.security.ProviderException
import java.security.Signature
import java.security.SignatureException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec
import org.junit.jupiter.api.Test

class KagemushaWalletAndroidPaymentKeyV1Test {
    private val keyStore = TestKeyStoreV1()
    private val environment = TestEnvironmentV1(File("."))
    private var intentState = KagemushaWalletAndroidIntentStateV1.DURABLE
    private val gateCalls = ArrayList<ByteArray>()
    private val paymentKey = KagemushaWalletAndroidPaymentKeyV1(keyStore, environment) { slot ->
        gateCalls += slot.copyOf()
        slot.fill(0)
        intentState
    }

    private var nextSlot = 1
    private fun slot(): ByteArray = ByteArray(32) { (nextSlot + it).toByte() }.also { nextSlot += 1 }
    private val challenge = ByteArray(32) { (0x40 + it).toByte() }
    private fun alias(slot: ByteArray) = kagemushaWalletAndroidAliasV1(slot)

    private fun platformCode(code: Int) = KagemushaWalletAndroidUnavailableV1.platform(code)

    private fun assertGenerationUnavailable(
        expected: KagemushaWalletAndroidUnavailableV1,
        result: KagemushaWalletAndroidKeyGenerationV1,
    ) {
        assertEquals(expected, assertIs<KagemushaWalletAndroidKeyGenerationV1.Unavailable>(result).reason)
    }

    @Test fun `alias is the fixed prefix and the lowercase hex of a nonzero 32-byte slot`() {
        val slot = ByteArray(32) { (0xa0 + it).toByte() }
        val alias = alias(slot)
        assertEquals("kgm-w1-" + slot.joinToString("") { "%02x".format(it.toInt() and 0xff) }, alias)
        assertEquals(alias, KagemushaWalletAndroidPlatformV1.paymentKeyAlias(slot))
        assertFailsWith<IllegalArgumentException> { alias(ByteArray(32)) }
        assertFailsWith<IllegalArgumentException> { alias(ByteArray(31) { 1 }) }
        assertFailsWith<IllegalArgumentException> { alias(ByteArray(33) { 1 }) }
    }

    @Test fun `probe is absent only for a null getKey and present with the certificate key`() {
        val slot = slot()
        assertSame(KagemushaWalletAndroidKeyProbeV1.Absent, paymentKey.probe(slot))
        val entry = keyStore.seed(alias(slot))
        val present = assertIs<KagemushaWalletAndroidKeyProbeV1.Present>(paymentKey.probe(slot))
        assertContentEquals(KagemushaP256Codec.uncompressedFromPublicKey(entry.pair.public), present.publicKeySec1())
        present.publicKeySec1().fill(0)
        assertContentEquals(KagemushaP256Codec.uncompressedFromPublicKey(entry.pair.public), present.publicKeySec1())
    }

    @Test fun `every Keystore error is unavailable and never absence`() {
        val slot = slot()
        keyStore.getKeyFailure = java.security.UnrecoverableKeyException("keystore2 SYSTEM_ERROR")
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot)).reason,
        )
        keyStore.getKeyFailure = java.security.UnrecoverableKeyException("invalidated")
            .apply { initCause(TestPermanentlyInvalidatedKeyV1()) }
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.PERMANENTLY_INVALIDATED,
            assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot)).reason,
        )
        keyStore.getKeyFailure = OutOfMemoryError("binder buffer")
        assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot))
        keyStore.getKeyFailure = null
        keyStore.seed(alias(slot))
        keyStore.nullCertificate = true
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE),
            assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot)).reason,
        )
        keyStore.nullCertificate = false
        keyStore.certificateFailure = IllegalStateException("certificate read failed")
        assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot))
        keyStore.certificateFailure = null
        keyStore.getKeyResult = javax.crypto.spec.SecretKeySpec(ByteArray(16), "AES")
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE,
            assertIs<KagemushaWalletAndroidKeyProbeV1.Unavailable>(paymentKey.probe(slot)).reason,
        )
    }

    @Test fun `a TEE key is generated with the exact non-authenticated spec and read back`() {
        environment.strongBox = false
        val slot = slot()
        val generated = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertEquals(KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT, generated.securityLevel)
        val spec = keyStore.generated.single()
        assertEquals(alias(slot), spec.alias)
        assertContentEquals(challenge, spec.challengeDigest())
        assertFalse(spec.strongBox)
        assertEquals("secp256r1", spec.curve)
        assertEquals(KeyProperties.PURPOSE_SIGN, spec.purposes)
        assertEquals(setOf(KeyProperties.DIGEST_SHA256), spec.digests)
        assertFalse(spec.userAuthenticationRequired)
        assertFalse(spec.unlockedDeviceRequired)
        assertFalse(spec.userPresenceRequired)
        assertFalse(spec.userConfirmationRequired)
        assertEquals(null, spec.maxUsageCount)
        val entry = keyStore.entries.getValue(alias(slot))
        assertContentEquals(KagemushaP256Codec.uncompressedFromPublicKey(entry.pair.public), generated.publicKeySec1())
        assertContentEquals(slot, gateCalls.single())
    }

    @Test fun `StrongBox is used when present and the secure-element profile never falls back`() {
        val first = slot()
        val strong = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            paymentKey.generate(first, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
        )
        assertEquals(KagemushaWalletAndroidSecurityLevelV1.STRONGBOX, strong.securityLevel)
        assertTrue(keyStore.generated.single().strongBox)

        keyStore.strongBoxAvailable = false
        val second = slot()
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_STRONGBOX_UNAVAILABLE),
            paymentKey.generate(second, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
        )
        assertEquals(listOf(true, true), keyStore.generated.map { it.strongBox })
        assertFalse(keyStore.entries.containsKey(alias(second)))

        environment.strongBox = false
        val third = slot()
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_STRONGBOX_UNAVAILABLE),
            paymentKey.generate(third, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
        )
        assertEquals(2, keyStore.generated.size)
    }

    @Test fun `the TEE fallback runs only after StrongBox refusal and a second definitive absence`() {
        keyStore.strongBoxAvailable = false
        val slot = slot()
        val tee = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertEquals(KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT, tee.securityLevel)
        assertEquals(listOf(true, false), keyStore.generated.map { it.strongBox })

        val uncertain = slot()
        keyStore.getKeyFailureAfterGenerations = keyStore.generated.size + 1
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            paymentKey.generate(uncertain, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertEquals(listOf(true, false, true), keyStore.generated.map { it.strongBox })
    }

    @Test fun `an existing entry is never replaced and its slot is never generated again`() {
        val slot = slot()
        val original = keyStore.seed(alias(slot))
        assertSame(
            KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent,
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias(slot)))
        keyStore.entries.clear()
        assertSame(
            KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent,
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `a Keystore error before generation never generates`() {
        val slot = slot()
        val original = keyStore.seed(alias(slot))
        // A containsAlias-style probe would read this error as absence and rebind the alias.
        keyStore.getKeyFailure = IllegalStateException("keystore2 binder failure")
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias(slot)))
    }

    @Test fun `generation requires a durable intent for the slot`() {
        val slot = slot()
        for ((state, code) in listOf(
            KagemushaWalletAndroidIntentStateV1.ABSENT to KagemushaWalletAndroidUnavailableV1.PLATFORM_INTENT_NOT_DURABLE,
            KagemushaWalletAndroidIntentStateV1.UNAVAILABLE to KagemushaWalletAndroidUnavailableV1.PLATFORM_INTENT_UNAVAILABLE,
        )) {
            intentState = state
            assertGenerationUnavailable(
                platformCode(code),
                paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
            )
        }
        val throwing = KagemushaWalletAndroidPaymentKeyV1(keyStore, environment) { throw IllegalStateException("bridge") }
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_INTENT_UNAVAILABLE),
            throwing.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertTrue(keyStore.generated.isEmpty())
        gateCalls.forEach { assertContentEquals(slot, it) }
        intentState = KagemushaWalletAndroidIntentStateV1.DURABLE
        assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
    }

    @Test fun `generation is refused while the application allows backup`() {
        val slot = slot()
        environment.flags = ApplicationInfo.FLAG_ALLOW_BACKUP
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_BACKUP_ENABLED),
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        environment.flags = 0
        environment.agent = "com.example.BackupAgent"
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_BACKUP_ENABLED),
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertTrue(keyStore.generated.isEmpty())
        assertTrue(gateCalls.isEmpty())
    }

    @Test fun `a failed generation retries only after another definitive absence`() {
        val slot = slot()
        keyStore.generateFailure = ProviderException("keymint failure")
        assertGenerationUnavailable(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_GENERATION_FAILED),
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        keyStore.generateFailure = null
        assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        assertEquals(2, keyStore.generated.size)
    }

    @Test fun `a generated key that differs from the request is unusable and stays in place`() {
        val edits: List<(KagemushaWalletAndroidKeyFactsV1) -> KagemushaWalletAndroidKeyFactsV1> = listOf(
            { it.copy(userAuthenticationRequired = true) },
            { it.copy(userPresenceRequired = true) },
            { it.copy(userConfirmationRequired = true) },
            { it.copy(remainingUsageCount = 1) },
            { it.copy(securityLevel = 1) },
            { it.copy(insideSecureHardware = false) },
            { it.copy(origin = 2) },
            { it.copy(purposes = 4 or 8) },
            { it.copy(digests = setOf("SHA-256", "NONE")) },
            { it.copy(keySize = 384) },
        )
        for (edit in edits) {
            val slot = slot()
            keyStore.factsEdit = edit
            assertGenerationUnavailable(
                KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE,
                paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
            )
            assertTrue(keyStore.entries.containsKey(alias(slot)))
            assertSame(
                KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent,
                paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
            )
        }
        keyStore.factsEdit = { it }
        val attested = slot()
        keyStore.attestedChallenge = ByteArray(32) { 7 }
        assertGenerationUnavailable(
            KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE,
            paymentKey.generate(attested, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
        )
        assertTrue(keyStore.entries.containsKey(alias(attested)))
    }

    @Test fun `key facts follow the API level of the readback`() {
        val modern = keyStore.facts(strongBox = true)
        assertTrue(kagemushaWalletAndroidFactsMatchV1(modern, KagemushaWalletAndroidSecurityLevelV1.STRONGBOX, exportable = false))
        assertFalse(kagemushaWalletAndroidFactsMatchV1(modern, KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT, exportable = false))
        assertFalse(kagemushaWalletAndroidFactsMatchV1(modern, KagemushaWalletAndroidSecurityLevelV1.STRONGBOX, exportable = true))
        keyStore.apiLevel = 30
        val legacy = keyStore.facts(strongBox = true)
        assertTrue(kagemushaWalletAndroidFactsMatchV1(legacy, KagemushaWalletAndroidSecurityLevelV1.STRONGBOX, exportable = false))
        assertTrue(kagemushaWalletAndroidFactsMatchV1(legacy, KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT, exportable = false))
        assertFalse(kagemushaWalletAndroidFactsMatchV1(legacy.copy(securityLevel = 2), KagemushaWalletAndroidSecurityLevelV1.STRONGBOX, exportable = false))
    }

    @Test fun `the hardware plan never selects the TEE for the secure-element profile`() {
        val profiles = KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT to KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_ONLY, kagemushaWalletAndroidHardwarePlanV1(profiles.first, 28, true))
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.REFUSE, kagemushaWalletAndroidHardwarePlanV1(profiles.first, 28, false))
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.REFUSE, kagemushaWalletAndroidHardwarePlanV1(profiles.first, 27, true))
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_THEN_TEE, kagemushaWalletAndroidHardwarePlanV1(profiles.second, 28, true))
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.TEE_ONLY, kagemushaWalletAndroidHardwarePlanV1(profiles.second, 28, false))
        assertEquals(KagemushaWalletAndroidHardwarePlanV1.TEE_ONLY, kagemushaWalletAndroidHardwarePlanV1(profiles.second, 24, true))
        assertEquals(KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT, KagemushaWalletAndroidKeyProfileV1.fromTag(1))
        assertEquals(KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE, KagemushaWalletAndroidKeyProfileV1.fromTag(2))
        assertEquals(null, KagemushaWalletAndroidKeyProfileV1.fromTag(0))
    }

    @Test fun `signing returns the platform DER over the exact preimage`() {
        val slot = slot()
        val entry = keyStore.seed(alias(slot))
        val preimage = "iroha:kagemusha:wallet:v1:receipt-body".toByteArray(Charsets.US_ASCII)
        val der = assertIs<KagemushaWalletAndroidSignatureV1.Der>(paymentKey.sign(slot, preimage)).der()
        assertTrue(Signature.getInstance("SHA256withECDSA").run {
            initVerify(entry.pair.public)
            update(preimage)
            verify(der)
        })
        val raw = KagemushaP256Codec.rawLowSFromStrictDer(der)
        assertTrue(KagemushaP256Codec.verifyRawLowS(KagemushaP256Codec.uncompressedFromPublicKey(entry.pair.public), preimage, raw))
        assertFailsWith<IllegalArgumentException> { paymentKey.sign(slot, ByteArray(0)) }
    }

    @Test fun `signing failures are unavailable and never touch the key`() {
        val slot = slot()
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEY_ABSENT),
            assertIs<KagemushaWalletAndroidSignatureV1.Unavailable>(paymentKey.sign(slot, byteArrayOf(1))).reason,
        )
        val entry = keyStore.seed(alias(slot))
        val cases = listOf(
            TestPermanentlyInvalidatedKeyV1() to KagemushaWalletAndroidUnavailableV1.PERMANENTLY_INVALIDATED,
            InvalidKeyException("key rejected") to KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE,
            SignatureException("operation failed") to KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE,
            ProviderException("keystore2 busy") to platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_SIGN_FAILED),
        )
        for ((failure, reason) in cases) {
            keyStore.signFailure = failure
            assertEquals(reason, assertIs<KagemushaWalletAndroidSignatureV1.Unavailable>(paymentKey.sign(slot, byteArrayOf(1))).reason)
        }
        keyStore.signFailure = null
        keyStore.getKeyFailure = IllegalStateException("keystore2 binder failure")
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            assertIs<KagemushaWalletAndroidSignatureV1.Unavailable>(paymentKey.sign(slot, byteArrayOf(1))).reason,
        )
        assertSame(entry, keyStore.entries.getValue(alias(slot)))
        assertEquals(0, keyStore.deleteCalls)
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `deletion reports the outcome of a fresh probe`() {
        val slot = slot()
        keyStore.seed(alias(slot))
        assertSame(KagemushaWalletAndroidRemoveV1.Removed, paymentKey.delete(slot))
        assertSame(KagemushaWalletAndroidRemoveV1.Removed, paymentKey.delete(slot))
        assertSame(
            KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent,
            paymentKey.generate(slot, challenge, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE),
        )
        val kept = slot()
        keyStore.seed(alias(kept))
        keyStore.deleteRemoves = false
        assertIs<KagemushaWalletAndroidRemoveV1.NotRemoved>(paymentKey.delete(kept))
        keyStore.deleteFailure = IllegalStateException("delete failed")
        keyStore.getKeyFailure = IllegalStateException("keystore2 binder failure")
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            assertIs<KagemushaWalletAndroidRemoveV1.Uncertain>(paymentKey.delete(kept)).reason,
        )
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `the attestation chain is exported only for a present key`() {
        val slot = slot()
        assertSame(KagemushaWalletAndroidAttestationChainV1.Absent, paymentKey.attestationChain(slot))
        val entry = keyStore.seed(alias(slot))
        val chain = assertIs<KagemushaWalletAndroidAttestationChainV1.Present>(paymentKey.attestationChain(slot))
        assertEquals(entry.chain!!.map { it.encoded.toList() }, chain.certificatesDer().map { it.toList() })
        keyStore.nullChain = true
        assertEquals(
            platformCode(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE),
            assertIs<KagemushaWalletAndroidAttestationChainV1.Unavailable>(paymentKey.attestationChain(slot)).reason,
        )
        keyStore.nullChain = false
        entry.chain = listOf(entry.chain!!.first())
        assertIs<KagemushaWalletAndroidAttestationChainV1.Unavailable>(paymentKey.attestationChain(slot))
        keyStore.getKeyFailure = IllegalStateException("keystore2 binder failure")
        assertIs<KagemushaWalletAndroidAttestationChainV1.Unavailable>(paymentKey.attestationChain(slot))
    }

    private fun KagemushaWalletAndroidKeyFactsV1.copy(
        insideSecureHardware: Boolean = this.insideSecureHardware,
        securityLevel: Int? = this.securityLevel,
        remainingUsageCount: Int? = this.remainingUsageCount,
        origin: Int = this.origin,
        purposes: Int = this.purposes,
        digests: Set<String> = this.digests,
        keySize: Int = this.keySize,
        userAuthenticationRequired: Boolean = this.userAuthenticationRequired,
        userPresenceRequired: Boolean = this.userPresenceRequired,
        userConfirmationRequired: Boolean = this.userConfirmationRequired,
    ) = KagemushaWalletAndroidKeyFactsV1(
        apiLevel, insideSecureHardware, securityLevel, remainingUsageCount, origin, purposes, digests, keySize,
        userAuthenticationRequired, userPresenceRequired, userConfirmationRequired,
    )
}
