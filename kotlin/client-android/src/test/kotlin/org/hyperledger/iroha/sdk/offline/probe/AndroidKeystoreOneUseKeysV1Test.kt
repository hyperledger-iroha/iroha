// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.probe

import java.security.KeyStoreException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.crypto.keystore.AndroidKeystoreAliasStateV1
import org.hyperledger.iroha.sdk.crypto.keystore.AndroidKeystoreUnavailableExceptionV1
import org.hyperledger.iroha.sdk.crypto.keystore.TestAndroidKeystoreV1
import org.junit.jupiter.api.Test

/** One-use diagnostic keys over a keystore2 model whose generation replaces any key under the alias. */
class AndroidKeystoreOneUseKeysV1Test {
    private val alias = "iroha_kagemusha_pixel6_testnet_" + "cd".repeat(32)
    private val challenge = ByteArray(32) { 5 }

    @Test fun `a Keystore error never generates and the existing key survives`() {
        val keyStore = TestAndroidKeystoreV1()
        val original = keyStore.seed(alias)
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox = true)
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { keys.aliasState(alias) }
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { keys.generate(alias, challenge) }
        assertTrue(keyStore.generated.isEmpty())
        assertEquals(0, keyStore.deleteCalls)
        assertSame(original, keyStore.entries.getValue(alias))
    }

    @Test fun `an occupied alias is never replaced`() {
        val keyStore = TestAndroidKeystoreV1()
        val original = keyStore.seed(alias)
        val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox = false)
        assertEquals(AndroidKeystoreAliasStateV1.PRESENT, keys.aliasState(alias))
        assertFailsWith<IllegalStateException> { keys.generate(alias, challenge) }
        assertTrue(keyStore.generated.isEmpty())
        assertSame(original, keyStore.entries.getValue(alias))
    }

    @Test fun `a definitive absence generates exactly one one-use key and reads it back`() {
        for (strongBox in listOf(false, true)) {
            val keyStore = TestAndroidKeystoreV1()
            val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox)
            assertEquals(AndroidKeystoreAliasStateV1.ABSENT, keys.aliasState(alias))
            val material = keys.generate(alias, challenge)
            val request = keyStore.generated.single()
            assertEquals(strongBox, request.strongBox)
            assertEquals(1, request.maxUsageCount)
            assertContentEquals(challenge, request.challenge())
            val entry = keyStore.entries.getValue(alias)
            assertContentEquals(uncompressedP256Sec1V1(entry.pair.public), material.publicKey)
            assertEquals(entry.chain.size, material.certificateChain.size)
            assertTrue(keys.sign(alias, byteArrayOf(1, 2, 3)).isNotEmpty())
            assertEquals(1, keyStore.signCalls)
        }
    }

    @Test fun `keystore1 cannot report absence, so nothing is generated`() {
        val keyStore = TestAndroidKeystoreV1(apiLevel = 30)
        val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox = false)
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { keys.aliasState(alias) }
        assertFailsWith<IllegalStateException> { keys.generate(alias, challenge) }
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `reading or signing an absent or unanswerable alias fails instead of succeeding`() {
        val keyStore = TestAndroidKeystoreV1()
        val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox = false)
        assertFailsWith<IllegalStateException> { keys.read(alias) }
        assertFailsWith<IllegalStateException> { keys.sign(alias, byteArrayOf(1)) }
        keyStore.seed(alias)
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { keys.read(alias) }
        assertFailsWith<AndroidKeystoreUnavailableExceptionV1> { keys.sign(alias, byteArrayOf(1)) }
        assertEquals(0, keyStore.signCalls)
    }

    @Test fun `deletion is unconditional and its errors are reported`() {
        val keyStore = TestAndroidKeystoreV1()
        val keys = AndroidKeystoreOneUseKeysV1(keyStore, strongBox = false)
        keys.delete(alias)
        assertEquals(1, keyStore.deleteCalls)
        keyStore.seed(alias)
        keyStore.deleteFailure = KeyStoreException("keystore2 binder failure")
        assertFailsWith<KeyStoreException> { keys.delete(alias) }
        assertTrue(alias in keyStore.entries)
    }
}
