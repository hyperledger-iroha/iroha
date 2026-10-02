// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/** Scripted framing/order tests only: no installed root, account, proof or JNI is created. */
class KagemushaInitialOrdinaryStartupV1Test {
    @Test fun `initial fixed phases use retained read id and accept distinct selected session id`() {
        val calls = mutableListOf<Pair<Int, Long>>()
        val endpoint = KagemushaOrdinaryNativeStartupEndpointV1 { phase, id, original ->
            assertContentEquals(ByteArray(0), original)
            calls += phase to id
            when (phase) { 1 -> reservation(); 6 -> selected(); else -> error("Unexpected phase") }
        }
        completeInitialOrdinaryStartupV1(endpoint)
        assertEquals(listOf(1 to 0L, 6 to 91L), calls)
    }

    @Test fun `missing reservation never invokes finish`() {
        var calls = 0
        assertFailsWith<IllegalStateException> {
            completeInitialOrdinaryStartupV1(KagemushaOrdinaryNativeStartupEndpointV1 { phase, _, _ ->
                assertEquals(1, phase); calls++; null
            })
        }
        assertEquals(1, calls)
    }

    @Test fun `malformed initial nonce ids versions phases and account bounds refuse before finish`() {
        val malformed = listOf(
            reservation().dropLast(1).toTypedArray(),
            reservation().also { it[0] = byteArrayOf(2, 0) },
            reservation().also { it[1] = byteArrayOf(6) },
            reservation().also { it[2] = ByteArray(8) },
            reservation().also { it[2] = ByteArray(7) },
            reservation().also { it[3] = ByteArray(32) },
            reservation().also { it[3] = ByteArray(31) { 1 } },
            reservation().also { it[4] = ByteArray(0) },
            reservation().also { it[4] = ByteArray(4097) },
            reservation().also { it[5] = ByteArray(0) },
            reservation().also { it[5] = ByteArray(4097) },
        )
        for (response in malformed) {
            var calls = 0
            assertFailsWith<IllegalArgumentException> {
                completeInitialOrdinaryStartupV1(KagemushaOrdinaryNativeStartupEndpointV1 { phase, _, _ ->
                    assertEquals(1, phase); calls++; response
                })
            }
            assertEquals(1, calls)
        }
    }

    @Test fun `null or malformed selected session acknowledgement never succeeds`() {
        val malformed: List<Array<ByteArray>?> = listOf(null,
            selected().dropLast(1).toTypedArray(),
            selected().also { it[0] = byteArrayOf(2, 0) },
            selected().also { it[1] = byteArrayOf(1) },
            selected().also { it[2] = ByteArray(8) },
            selected().also { it[2] = ByteArray(9) })
        for (response in malformed) {
            var calls = 0
            val attempt = {
                completeInitialOrdinaryStartupV1(KagemushaOrdinaryNativeStartupEndpointV1 { phase, _, _ ->
                    calls++; if (phase == 1) reservation() else response
                })
            }
            if (response == null) assertFailsWith<IllegalStateException> { attempt() }
            else assertFailsWith<IllegalArgumentException> { attempt() }
            assertEquals(2, calls)
        }
    }

    @Test fun `startup linkage failure retains original cause and never retries`() {
        for (phaseThatFails in listOf(1, 6)) {
            val originalFailure = UnsatisfiedLinkError("No admitted startup endpoint")
            val calls = mutableListOf<Int>()
            val failure = assertFailsWith<IllegalStateException> {
                completeInitialOrdinaryStartupV1(KagemushaOrdinaryNativeStartupEndpointV1 { phase, _, _ ->
                    calls += phase
                    if (phase == phaseThatFails) throw originalFailure
                    reservation()
                })
            }
            assertSame(originalFailure, failure.cause)
            assertEquals(if (phaseThatFails == 1) listOf(1) else listOf(1, 6), calls)
        }
    }

    @Test fun `public initial dispatch rejects caller endpoint before any callback or loading`() {
        var callbacks = 0
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorBridgeV1.selectInitialOrdinaryAccount(
                KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> callbacks++; error("Forbidden callback") })
        }
        assertEquals(0, callbacks)
    }

    @Test fun `successful initial selection is once only and precedes every original open`() {
        val gate = KagemushaInitialOrdinaryStartupGateV1()
        val calls = mutableListOf<String>()
        gate.select { calls += "select" }
        assertFailsWith<IllegalStateException> { gate.select { calls += "repeat" } }
        assertEquals("initial", gate.open { calls += "open"; "initial" })
        assertEquals("recovered", gate.open { calls += "recover"; "recovered" })
        assertEquals(listOf("select", "open", "recover"), calls)
    }

    @Test fun `failed loader or startup freezes open and forbids a replacement selection`() {
        val failures: List<Throwable> = listOf(UnsatisfiedLinkError("Load failed"), IllegalStateException("No Native root"))
        for (failure in failures) {
            val gate = KagemushaInitialOrdinaryStartupGateV1()
            var starts = 0; var opens = 0
            val caught = assertFailsWith<Throwable> { gate.select { starts++; throw failure } }
            assertSame(failure, caught)
            assertFailsWith<IllegalStateException> { gate.select { starts++ } }
            assertFailsWith<IllegalStateException> { gate.open { opens++ } }
            assertEquals(1, starts); assertEquals(0, opens)
        }
    }

    @Test fun `earlier uncertain Core open cannot be rescued by initial selection`() {
        val gate = KagemushaInitialOrdinaryStartupGateV1()
        val original = IllegalStateException("Original open uncertain")
        val caught = assertFailsWith<IllegalStateException> { gate.open { throw original } }
        assertSame(original, caught)
        var selections = 0
        assertFailsWith<IllegalStateException> { gate.select { selections++ } }
        assertEquals(0, selections)
    }

    @Test fun `nested open or replacement selection cannot escape active initial selection`() {
        val gate = KagemushaInitialOrdinaryStartupGateV1()
        var forbidden = 0
        gate.select {
            assertFailsWith<IllegalStateException> { gate.open { forbidden++ } }
            assertFailsWith<IllegalStateException> { gate.select { forbidden++ } }
        }
        gate.open { forbidden++ }
        assertEquals(1, forbidden)
    }

    @Test fun `parallel production open waits for completed initial selection`() {
        val gate = KagemushaInitialOrdinaryStartupGateV1()
        val entered = CountDownLatch(1); val release = CountDownLatch(1)
        val opening = CountDownLatch(1); val opened = CountDownLatch(1)
        val failure = AtomicReference<Throwable?>()
        val selection = thread {
            try { gate.select { entered.countDown(); assertTrue(release.await(5, TimeUnit.SECONDS)) } }
            catch (error: Throwable) { failure.compareAndSet(null, error) }
        }
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        val open = thread {
            try { opening.countDown(); gate.open { opened.countDown() } }
            catch (error: Throwable) { failure.compareAndSet(null, error) }
        }
        try {
            assertTrue(opening.await(5, TimeUnit.SECONDS))
            assertFalse(opened.await(100, TimeUnit.MILLISECONDS))
        } finally { release.countDown() }
        selection.join(5000); open.join(5000)
        assertFalse(selection.isAlive); assertFalse(open.isAlive)
        failure.get()?.let { throw it }
        assertEquals(0L, opened.count)
    }

    private fun reservation(): Array<ByteArray> = arrayOf(
        byteArrayOf(1, 0), byteArrayOf(1), le64(91), ByteArray(32) { 1 }, byteArrayOf(1), byteArrayOf(2))
    private fun selected(): Array<ByteArray> = arrayOf(byteArrayOf(1, 0), byteArrayOf(6), le64(117))
    private fun le64(value: Long) = ByteArray(8) { (value ushr (it * 8)).toByte() }
}
