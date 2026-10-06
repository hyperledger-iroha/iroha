// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.File
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import org.hyperledger.iroha.sdk.testing.JvmApiInventory

/** Managed ownership/value contracts; real authenticated artifact execution is a separate gate. */
class KagemushaWalletV1Test {
    @TempDir lateinit var directory: File

    @Test fun `retained output and callback bytes are defensive and redacted`() {
        val bytes = byteArrayOf(0, -1, 0, 7)
        val result = KagemushaWalletCallV1(1, -1, 0, 7, 2, 0, bytes)
        val callback = KagemushaWalletNativeReplyV1(0, bytes = bytes)
        bytes[0] = 9
        result.bytes()[1] = 9
        callback.bytes()[1] = 9
        assertContentEquals(byteArrayOf(0, -1, 0, 7), result.bytes())
        assertContentEquals(result.bytes(), callback.bytes())
        assertTrue(result.toString().contains("[REDACTED]"))
        for (status in 0..10) {
            val value = KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, if (status == 1 || status == 10) byteArrayOf(1) else byteArrayOf())
            assertEquals(status, value.status)
        }
    }
    @Test fun `native callback reasons retain their distinction`() {
        val values = listOf(KagemushaWalletAndroidUnavailableV1.LOCKED, KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK,
            KagemushaWalletAndroidUnavailableV1.BUSY, KagemushaWalletAndroidUnavailableV1.io(5), KagemushaWalletAndroidUnavailableV1.platform(8),
            KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE, KagemushaWalletAndroidUnavailableV1.PERMANENTLY_INVALIDATED)
        values.forEachIndexed { index, reason ->
            val unavailable = KagemushaWalletNativeReplyV1.unavailable(reason)
            val uncertain = KagemushaWalletNativeReplyV1.unavailable(reason, 4)
            assertEquals(index, unavailable.reason); assertEquals(reason.code, unavailable.code)
            assertEquals(2, unavailable.tag); assertEquals(4, uncertain.tag)
        }
    }
    @Test fun `factory rejects invalid incarnation before loading native code`() {
        val platform = KagemushaWalletAndroidPlatformV1.create(TestEnvironmentV1(directory), TestKeyStoreV1())
        val id = ByteArray(32) { 1 }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletV1.open(platform, ByteArray(31), id, id, id) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletV1.open(platform, id, id, id, ByteArray(32)) }
    }
    @Test fun `native API contains only opaque state machine calls`() {
        val type = JvmApiInventory.read(KagemushaWalletNativeV1::class.java)
        assertEquals(setOf("revision", "open", "close", "activity", "call", "snapshot"), type.methods.filter { it.flags and 0x0100 != 0 }.map { it.name }.toSet())
        assertEquals(-4, KagemushaWalletExceptionV1.ARTIFACTS_UNAVAILABLE)
    }
}
