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
        for (status in 0..11) {
            val value = KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, if (status == 1 || status == 10) byteArrayOf(1) else byteArrayOf())
            assertEquals(status, value.status)
        }
    }
    @Test fun `malformed native output never becomes completion`() {
        for ((status, bytes) in listOf(
            1 to byteArrayOf(), 10 to byteArrayOf(), 2 to byteArrayOf(1),
            12 to byteArrayOf(), -4 to byteArrayOf(1), 1 to ByteArray(10_001),
        )) {
            val error = assertFailsWith<KagemushaWalletExceptionV1> {
                KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, bytes)
            }
            assertEquals(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT, error.status)
        }
        for (status in 0..11) {
            if (status != 1 && status != 10) {
                assertFailsWith<KagemushaWalletExceptionV1> {
                    KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, byteArrayOf(1))
                }
            }
        }
        assertEquals(10_000, KagemushaWalletCallV1(1, -1, 0, 0, 0, 0, ByteArray(10_000)).bytes().size)
    }
    @Test fun `native failure metadata remains distinct from a completion`() {
        val failure = KagemushaWalletCallV1(-4, 3, 5, 0, 0, 0, byteArrayOf())
        assertEquals(-4, failure.status)
        assertEquals(3, failure.reason)
        assertEquals(5, failure.platformCode)
        assertTrue(failure.bytes().isEmpty())
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
        assertEquals(setOf("revision", "open", "close", "activity", "call", "snapshot", "execute"), type.methods.filter { it.flags and 0x0100 != 0 }.map { it.name }.toSet())
        assertEquals(-4, KagemushaWalletExceptionV1.ARTIFACTS_UNAVAILABLE)
    }
    @Test fun `typed lifecycle inputs bound originals and preserve unsigned scalar bits`() {
        val id = ByteArray(32) { 1 }
        for (selector in 0..9) {
            val limits = when (selector) {
                0 -> intArrayOf(512, 16_384, 0)
                1 -> intArrayOf(10_000, 0, 0)
                2 -> intArrayOf(10_000, 1_024, 10_000)
                5 -> intArrayOf(65_536 * 34 + 512, 10_000, 0)
                6 -> intArrayOf(512, 10_000, 0)
                7 -> intArrayOf(8_192, 10_000, 0)
                9 -> intArrayOf(0, 0, 0)
                else -> intArrayOf(1_024, 10_000, 0)
            }
            val original = limits.map { if (it == 0) byteArrayOf() else byteArrayOf(7) }
            val amount = if (selector == 8) KagemushaWalletUInt128V1(-1, -1) else KagemushaWalletUInt128V1(0, 0)
            val input = KagemushaWalletOperationInputV1(id, selector, amount, original[0], original[1], original[2])
            assertEquals(selector, input.selector)
            assertEquals(amount, input.amount)
            assertContentEquals(id, input.requestId())
            assertContentEquals(original[0], input.first())
            assertContentEquals(original[1], input.second())
            assertContentEquals(original[2], input.third())
            input.requestId()[0] = 0
            assertContentEquals(id, input.requestId())
            assertTrue(input.toString().contains("[REDACTED]"))
            for (index in 0..2) {
                val changed = original.toMutableList()
                changed[index] = ByteArray(limits[index] + 1)
                assertFailsWith<IllegalArgumentException> { KagemushaWalletOperationInputV1(id, selector, amount, changed[0], changed[1], changed[2]) }
            }
            for (bytes in original) if (bytes.isNotEmpty()) bytes[0] = 0
            if (limits[0] != 0) assertContentEquals(byteArrayOf(7), input.first())
            if (limits[1] != 0) assertContentEquals(byteArrayOf(7), input.second())
            if (limits[2] != 0) assertContentEquals(byteArrayOf(7), input.third())
        }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletOperationInputV1(ByteArray(32), 9) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletOperationInputV1(id, 10) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletOperationInputV1(id, 8) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletOperationInputV1(id, 8, KagemushaWalletUInt128V1(1, 0), byteArrayOf(1)) }
        assertEquals(8, KagemushaWalletOperationInputV1(id, 8, KagemushaWalletUInt128V1(1, 0)).selector)
        assertEquals(listOf(3, 4, 5, 6, 7), KagemushaWalletRefreshKindV1.values().map { it.selector })
    }

}
