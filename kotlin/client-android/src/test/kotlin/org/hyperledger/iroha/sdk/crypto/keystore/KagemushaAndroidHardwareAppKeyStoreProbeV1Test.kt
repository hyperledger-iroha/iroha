// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import java.security.KeyStoreException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * Existence decisions of the app-key adapter over a keystore2 model whose generation replaces any
 * key under the alias. Synthetic attestation only; no hardware is qualified by this suite.
 */
class KagemushaAndroidHardwareAppKeyStoreProbeV1Test {
    private val alias = "kagemusha-app-enrollment-v1-" + "ab".repeat(32)
    private val challenge = ByteArray(32) { (it + 1).toByte() }
    private val keyStore = TestAndroidKeystoreV1(apiLevel = 31)
    private var strongBoxFeature = true
    private val appKeys = KagemushaAndroidHardwareAppKeyStoreV1(keyStore) { strongBoxFeature }

    private fun issue(policy: KagemushaAndroidAppKeyHardwarePolicyV1 = KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) =
        appKeys.issueExact(alias, challenge, policy) {}

    private fun recover(policy: KagemushaAndroidAppKeyHardwarePolicyV1 = KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) =
        appKeys.recoverExact(alias, challenge, policy) {}

    @Test fun `a Keystore error before issuance never generates and the existing key survives`() {
        val original = keyStore.seed(alias, challenge)
        // A containsAlias probe would read this error as absence and rebind the alias.
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { issue() }
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias))
    }

    @Test fun `an occupied alias is recovered, never regenerated`() {
        val original = keyStore.seed(alias, challenge)
        for (policy in KagemushaAndroidAppKeyHardwarePolicyV1.values()) {
            val refused = assertFailsWith<IllegalStateException> { issue(policy) }
            assertEquals("Original app key already exists; recover it", refused.message)
        }
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias))
        val recovered = assertNotNull(recover())
        assertContentEquals(testKeystoreSec1V1(original.pair.public), recovered.publicKeySec1())
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `a definitive absence issues exactly one StrongBox key`() {
        val evidence = issue()
        assertEquals(1, keyStore.generated.size)
        val request = keyStore.generated.single()
        assertEquals(alias, request.alias)
        assertTrue(request.strongBox)
        assertNull(request.maxUsageCount)
        assertContentEquals(challenge, request.challenge())
        assertEquals(KeyProperties.SECURITY_LEVEL_STRONGBOX, evidence.securityLevel)
        assertContentEquals(testKeystoreSec1V1(keyStore.entries.getValue(alias).pair.public), evidence.publicKeySec1())
        // A retry of the same original sees the key and refuses instead of replacing it.
        assertFailsWith<IllegalStateException> { issue() }
        assertEquals(1, keyStore.generated.size)
    }

    @Test fun `TEE-only and devices without StrongBox generate in the TEE`() {
        strongBoxFeature = false
        assertEquals(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, issue().securityLevel)
        assertFalse(keyStore.generated.single().strongBox)
        val teeOnly = TestAndroidKeystoreV1()
        val evidence = KagemushaAndroidHardwareAppKeyStoreV1(teeOnly) { true }
            .issueExact(alias, challenge, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY) {}
        assertEquals(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, evidence.securityLevel)
        assertFalse(teeOnly.generated.single().strongBox)
    }

    @Test fun `StrongBox fallback to the TEE needs a second definitive absence`() {
        keyStore.strongBoxAvailable = false
        keyStore.getKeyFailureAfterGenerations = 1
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { issue() }
        assertEquals(listOf(true), keyStore.generated.map { it.strongBox })
        assertTrue(keyStore.entries.isEmpty())
    }

    @Test fun `a StrongBox failure that left a key is never followed by a TEE replacement`() {
        keyStore.strongBoxAvailable = false
        keyStore.strongBoxFailureLeavesKey = true
        assertFailsWith<IllegalStateException> { issue() }
        assertEquals(listOf(true), keyStore.generated.map { it.strongBox })
        val left = keyStore.entries.getValue(alias)
        assertSame(left.request, keyStore.generated.single())
    }

    @Test fun `a definite StrongBox-unavailable result falls back only under TEE_OR_STRONGBOX`() {
        keyStore.strongBoxAvailable = false
        val evidence = issue()
        assertEquals(listOf(true, false), keyStore.generated.map { it.strongBox })
        assertEquals(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, evidence.securityLevel)

        val strongOnly = TestAndroidKeystoreV1().apply { strongBoxAvailable = false }
        assertFailsWith<IllegalStateException> {
            KagemushaAndroidHardwareAppKeyStoreV1(strongOnly) { true }
                .issueExact(alias, challenge, KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) {}
        }
        assertEquals(listOf(true), strongOnly.generated.map { it.strongBox })
    }

    @Test fun `recovery reports absence only from a definitive keystore2 answer`() {
        assertNull(recover())
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { recover() }
        keyStore.getKeyFailure = null
        keyStore.seed(alias, challenge)
        assertIs<KagemushaAndroidHardwareAppKeyEvidenceV1>(recover())
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `a stale original challenge is rejected without replacing the key`() {
        val original = keyStore.seed(alias, ByteArray(32) { 7 })
        assertFailsWith<IllegalArgumentException> { recover() }
        assertFailsWith<IllegalStateException> { issue() }
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias))
    }

    @Test fun `keystore1 refuses every call before touching the Keystore`() {
        for (api in listOf(26, 28, 30)) {
            val legacy = TestAndroidKeystoreV1(apiLevel = api)
            val original = legacy.seed(alias, challenge)
            val legacyKeys = KagemushaAndroidHardwareAppKeyStoreV1(legacy) { true }
            assertFailsWith<IllegalStateException> {
                legacyKeys.issueExact(alias, challenge, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) {}
            }
            assertFailsWith<IllegalStateException> {
                legacyKeys.recoverExact(alias, challenge, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) {}
            }
            assertEquals(0, legacy.getKeyCalls)
            assertTrue(legacy.generated.isEmpty())
            assertSame(original, legacy.entries.getValue(alias))
        }
    }

    @Test fun `a stale caller guard stops before generation`() {
        var current = true
        assertFailsWith<IllegalStateException> {
            appKeys.issueExact(alias, challenge, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) {
                check(current) { "stale" }
                current = false
            }
        }
        assertTrue(keyStore.generated.isEmpty())
    }
}
