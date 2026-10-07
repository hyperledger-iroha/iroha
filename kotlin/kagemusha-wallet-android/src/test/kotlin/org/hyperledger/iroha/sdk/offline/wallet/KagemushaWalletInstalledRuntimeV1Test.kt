// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
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

    @Test fun `all seven originals are mandatory including the complete financial trio`() {
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

    @Test fun `installed enrollment failures retain the same pre-admission sequence`() {
        val failure = IllegalStateException("native unavailable")
        val admission = KagemushaWalletInstalledAdmissionV1 { }
        var calls = 0
        assertEquals(failure, assertFailsWith<IllegalStateException> {
            admission.enrollment { calls++; throw failure }
        })
        assertEquals("retained original", admission.enrollment { calls++; "retained original" })
        assertEquals(2, calls)
        admission.start(List(4) { byteArrayOf(1) })
        assertFailsWith<IllegalStateException> { admission.enrollment { calls++ } }
        assertEquals(2, calls)
    }

    @Test fun `admission attempt permanently closes installed enrollment access`() {
        val admission = KagemushaWalletInstalledAdmissionV1 { }
        admission.start(List(4) { byteArrayOf(1) })
        assertFailsWith<IllegalStateException> { admission.start(List(4) { byteArrayOf(1) }) }
        var reachedNative = false
        assertFailsWith<IllegalStateException> { admission.enrollment { reachedNative = true } }
        assertEquals(false, reachedNative)
    }

    @Test fun `ordinary refusal retries exact original frames without reopening enrollment`() {
        val admission = KagemushaWalletInstalledAdmissionV1 { }
        val original = List(4) { byteArrayOf(it.toByte(), 7) }
        admission.start(original)
        original.forEach { it.fill(99) }
        admission.failed()
        val same = List(4) { byteArrayOf(it.toByte(), 7) }
        assertFailsWith<IllegalStateException> { admission.start(List(4) { byteArrayOf(2) }) }
        var reachedNative = false
        assertFailsWith<IllegalStateException> { admission.enrollment { reachedNative = true } }
        assertEquals(false, reachedNative)
        admission.start(same)
        assertFailsWith<IllegalStateException> { admission.start(same) }
        admission.failed()
        admission.start(same)
        admission.completed()
        assertFailsWith<IllegalStateException> { admission.start(same) }
        assertFailsWith<IllegalStateException> { admission.failed() }
    }

    @Test fun `retired installed owner cannot enroll or start admission`() {
        var retired = false
        val admission = KagemushaWalletInstalledAdmissionV1 { check(!retired) { "retired" } }
        assertEquals(7, admission.enrollment { 7 })
        retired = true
        var reachedNative = false
        assertFailsWith<IllegalStateException> { admission.enrollment { reachedNative = true } }
        assertFailsWith<IllegalStateException> { admission.start(List(4) { byteArrayOf(1) }) }
        assertEquals(false, reachedNative)
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

    @Test fun `bare exceptional cancellation really activates a cancellation observer`() {
        val result = CompletableFuture<String>()
        var observations = 0
        result.whenComplete { _, _ -> if (result.isCancelled) observations++ }
        assertTrue(result.completeExceptionally(CancellationException("ordinary producer refusal")))
        assertTrue(result.isCancelled)
        assertEquals(1, observations)
    }

    @Test fun `ordinary cancellation preserves exact failure and permits the same admission sequence`() {
        val frames = List(4) { byteArrayOf(it.toByte(), 7) }
        val admission = KagemushaWalletInstalledAdmissionV1 { }
        admission.start(frames)
        val original = CancellationException("ordinary producer refusal")
        val evidence = IllegalStateException("original diagnostic")
        original.addSuppressed(evidence)
        val result = CompletableFuture<String>()
        var observations = 0
        result.whenComplete { _, _ -> if (result.isCancelled) observations++ }
        admission.failed()
        assertTrue(completeInstalledOpenFailureV1(result, original))
        assertFalse(result.isCancelled)
        assertTrue(result.isCompletedExceptionally)
        assertEquals(0, observations)
        assertSame(original, assertFailsWith<CompletionException> { result.join() }.cause)
        assertSame(evidence, original.suppressed.single())
        admission.start(frames)
        admission.completed()
        assertFailsWith<IllegalStateException> { admission.start(frames) }
    }

    @Test fun `cancelled producer future does not cancel the returned admission future`() {
        val producer = CompletableFuture<ByteArray>()
        assertTrue(producer.cancel(false))
        val ordinary = assertFailsWith<CancellationException> { producer.join() }
        val result = CompletableFuture<String>()
        var observations = 0
        result.whenComplete { _, _ -> if (result.isCancelled) observations++ }
        assertTrue(completeInstalledOpenFailureV1(result, ordinary))
        assertFalse(result.isCancelled)
        assertEquals(0, observations)
        assertSame(ordinary, assertFailsWith<CompletionException> { result.join() }.cause)
    }

    @Test fun `explicit returned future cancellation still wins over queued ordinary refusal`() {
        val result = CompletableFuture<String>()
        var observations = 0
        result.whenComplete { _, _ -> if (result.isCancelled) observations++ }
        assertTrue(result.cancel(false))
        assertTrue(result.isCancelled)
        assertEquals(1, observations)
        assertFalse(completeInstalledOpenFailureV1(result, CancellationException("late producer refusal")))
        assertTrue(result.isCancelled)
        assertEquals(1, observations)
        assertFailsWith<CancellationException> { result.join() }
    }

    @Test fun `completed ordinary refusal cannot later become explicit cancellation`() {
        val result = CompletableFuture<String>()
        assertTrue(completeInstalledOpenFailureV1(result, CancellationException("ordinary guard refusal")))
        assertFalse(result.cancel(false))
        assertFalse(result.isCancelled)
        val other = IllegalStateException("ordinary Native refusal")
        val failed = CompletableFuture<String>()
        assertTrue(completeInstalledOpenFailureV1(failed, other))
        assertSame(other, assertFailsWith<CompletionException> { failed.join() }.cause)
    }
}
