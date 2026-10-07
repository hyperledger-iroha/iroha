// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.testing.JvmApiInventory
import org.junit.jupiter.api.Test

/** Managed installation transport only; does not qualify Native signatures, sources or phones. */
class KagemushaWalletInstalledRuntimeV1Test {
    private fun originals(values: List<ByteArray>) = KagemushaWalletInstallationOriginalsV1(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6])

    @Test fun `all seven exact originals retain native role order and defensive copies`() {
        val values = List(7) { byteArrayOf(it.toByte(), 0, -1) }
        val retained = originals(values)
        values.forEach { it.fill(19) }
        val firstRead = retained.frames()
        firstRead.forEachIndexed { index, bytes ->
            assertContentEquals(byteArrayOf(index.toByte(), 0, -1), bytes)
            bytes.fill(23)
        }
        retained.frames().forEachIndexed { index, bytes ->
            assertContentEquals(byteArrayOf(index.toByte(), 0, -1), bytes)
        }
        assertTrue(retained.toString().contains("[REDACTED]"))
    }

    @Test fun `every original is mandatory and financial absence is refused`() {
        for (role in 0..6) {
            val values = MutableList(7) { byteArrayOf(1) }
            values[role] = byteArrayOf()
            assertFailsWith<IllegalArgumentException> { originals(values) }
        }
        for (presence in 0..7) {
            val values = MutableList(7) { byteArrayOf(1) }
            listOf(3, 4, 6).forEachIndexed { bit, role ->
                if (presence and (1 shl bit) == 0) values[role] = byteArrayOf()
            }
            if (presence == 7) {
                // Retaining these bytes conveys no authentication or readiness.
                val retained = originals(values).frames()
                values.forEachIndexed { role, bytes -> assertContentEquals(bytes, retained[role]) }
            } else assertFailsWith<IllegalArgumentException> { originals(values) }
        }
    }

    @Test fun `each role is bounded before originals are retained`() {
        val bounds = intArrayOf(8 * 1024 * 1024, 2048, 128 * 1024,
            16 * 1024 * 1024 + 65536, 16 * 1024 * 1024, 64 * 1024 * 1024, 4096)
        bounds.forEachIndexed { role, bound ->
            val values = MutableList(7) { byteArrayOf(1) }
            values[role] = ByteArray(bound + 1)
            assertFailsWith<IllegalArgumentException> { originals(values) }
        }
    }

    @Test fun `native refusal is preserved and malformed handles cannot create runtime ownership`() {
        for (status in listOf(-1, -2, -3, -4, -7, Int.MIN_VALUE)) {
            assertEquals(status, assertFailsWith<KagemushaWalletExceptionV1> {
                installationRuntimeHandle(status.toLong())
            }.status)
        }
        for (invalid in listOf(0L, Int.MIN_VALUE.toLong() - 1, Long.MIN_VALUE)) {
            assertEquals(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT,
                assertFailsWith<KagemushaWalletExceptionV1> { installationRuntimeHandle(invalid) }.status)
        }
        assertEquals(1L, installationRuntimeHandle(1))
        assertEquals(Long.MAX_VALUE, installationRuntimeHandle(Long.MAX_VALUE))
    }

    @Test fun `compiled JNI declaration accepts only platform and seven original arrays`() {
        val methods = JvmApiInventory.read(KagemushaWalletInstalledRuntimeNativeV1::class.java)
            .methods.filter { it.isNative }
        assertEquals(1, methods.size)
        val entry = methods.single()
        assertEquals("installRuntime", entry.name)
        assertTrue(entry.isStatic)
        assertEquals("(Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletAndroidPlatformV1;[B[B[B[B[B[B[B)J",
            entry.descriptor)
    }
}
