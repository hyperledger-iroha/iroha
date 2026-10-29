// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.IOException
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException
import kotlin.test.*
import org.junit.jupiter.api.Test

class KagemushaWalletCallObservationV1Test {
    private fun refusal() = KagemushaWalletCallV1(-7, -1, 0, 0, 0, 0, byteArrayOf())

    @Test fun `successful native calls are silent and retain exact reply and original bytes`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        val bytes = ByteArray(32) { (it + 1).toByte() }
        val reply = KagemushaWalletCallV1(KagemushaWalletCallV1.ACCOUNT_CHALLENGE, -1, 0, 1, 0, 0, bytes)
        var calls = 0
        for (boundary in WalletCallBoundaryV1.values()) {
            assertSame(reply, observer.call(boundary, 12) { calls++; reply })
        }
        assertEquals(WalletCallBoundaryV1.values().size, calls)
        assertContentEquals(bytes, reply.bytes())
        assertTrue(lines.isEmpty())
    }

    @Test fun `enrollment refusals identify each recovery selector without changing native fields`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        val reply = refusal()
        for (selector in listOf(12, 16, 18, 6)) {
            assertSame(reply, observer.call(WalletCallBoundaryV1.ENROLLMENT, selector) { reply })
        }
        assertEquals(listOf(12, 16, 18, 6).map {
            "event=refused operation=enrollment selector=$it status=-7 reason=-1 platform_code=0"
        }, lines)
    }

    @Test fun `opening refusals and missing native replies retain their original result`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        val reply = refusal()
        assertSame(reply, observer.call(WalletCallBoundaryV1.OPEN_BEGIN) { reply })
        assertSame(reply, observer.call(WalletCallBoundaryV1.OPEN_FINISH) { reply })
        assertNull(observer.call(WalletCallBoundaryV1.ENROLLMENT, 18) { null })
        assertEquals(listOf(
            "event=refused operation=open_begin status=-7 reason=-1 platform_code=0",
            "event=refused operation=open_finish status=-7 reason=-1 platform_code=0",
            "event=refused operation=enrollment selector=18 status=-100 reason=-1 platform_code=0",
        ), lines)
    }

    @Test fun `unexpected selectors are bounded and nonenrollment boundaries omit selectors`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        for (selector in listOf(null, -1, 22, Int.MAX_VALUE)) {
            observer.call(WalletCallBoundaryV1.ENROLLMENT, selector) { refusal() }
        }
        assertEquals(4, lines.size)
        assertTrue(lines.all { it == "event=refused operation=enrollment selector=unknown status=-7 reason=-1 platform_code=0" })
        lines.clear()
        observer.call(WalletCallBoundaryV1.OPEN_FINISH, Int.MAX_VALUE) { refusal() }
        assertEquals(listOf("event=refused operation=open_finish status=-7 reason=-1 platform_code=0"), lines)
    }

    @Test fun `thrown failures keep their identity while diagnostics omit messages and arbitrary class names`() {
        class PrivateAccountFailure : RuntimeException("private account, key and original")
        val failures = listOf(
            CancellationException("private") to "cancelled",
            SecurityException("private") to "security",
            IOException("private") to "io",
            IllegalArgumentException("private") to "invalid_argument",
            IllegalStateException("private") to "invalid_state",
            UnsatisfiedLinkError("private") to "linkage",
            PrivateAccountFailure() to "other",
        )
        for ((failure, category) in failures) {
            val lines = mutableListOf<String>()
            val observer = WalletCallObservationV1(lines::add)
            assertSame(failure, assertFails { observer.call(WalletCallBoundaryV1.OPEN_BEGIN) { throw failure } })
            assertEquals(listOf("event=refused operation=open_begin failure=$category"), lines)
        }
        val native = KagemushaWalletExceptionV1(-5, 3, 13)
        val lines = mutableListOf<String>()
        assertSame(native, assertFails {
            WalletCallObservationV1(lines::add).call(WalletCallBoundaryV1.ENROLLMENT, 16) { throw native }
        })
        assertEquals(listOf("event=refused operation=enrollment selector=16 failure=native status=-5 reason=3 platform_code=13"), lines)
    }

    @Test fun `signing success keeps the exact existing future and emits nothing`() {
        val lines = mutableListOf<String>()
        val future = CompletableFuture<ByteArray>()
        val observer = WalletCallObservationV1(lines::add)
        var calls = 0
        assertSame(future, observer.accountSignature { calls++; future })
        val signature = ByteArray(64) { 7 }
        future.complete(signature)
        assertEquals(1, calls)
        assertSame(signature, future.join())
        assertTrue(lines.isEmpty())
    }

    @Test fun `synchronous and asynchronous signer failures retain the exact original outcomes`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        val failure = IllegalStateException("private account seed")
        assertSame(failure, assertFails { observer.accountSignature<ByteArray> { throw failure } })
        val future = CompletableFuture<ByteArray>()
        assertSame(future, observer.accountSignature { future })
        val delivered = CompletionException(ExecutionException(failure))
        future.completeExceptionally(delivered)
        assertSame(delivered, assertFailsWith<CompletionException> { future.join() })
        assertEquals(List(2) { "event=refused operation=account_signature failure=invalid_state" }, lines)
    }

    @Test fun `future unwrapping is bounded and subclasses remain opaque`() {
        val lines = mutableListOf<String>()
        val observer = WalletCallObservationV1(lines::add)
        var deep: Throwable = KagemushaWalletExceptionV1(-7)
        repeat(9) { deep = CompletionException(deep) }
        val first = CompletableFuture<ByteArray>()
        observer.accountSignature { first }
        first.completeExceptionally(deep)
        class Opaque : CompletionException("private", IllegalStateException("private"))
        val second = CompletableFuture<ByteArray>()
        observer.accountSignature { second }
        second.completeExceptionally(Opaque())
        assertEquals(List(2) { "event=refused operation=account_signature failure=other" }, lines)
    }

    @Test fun `diagnostic failures cannot replace replies exceptions or signing futures`() {
        val observer = WalletCallObservationV1 { throw AssertionError("logging unavailable") }
        val reply = refusal()
        assertSame(reply, observer.call(WalletCallBoundaryV1.ENROLLMENT, 18) { reply })
        assertNull(observer.call(WalletCallBoundaryV1.OPEN_FINISH) { null })
        val failure = IOException("original")
        assertSame(failure, assertFails { observer.call(WalletCallBoundaryV1.OPEN_BEGIN) { throw failure } })
        assertSame(failure, assertFails { observer.accountSignature<ByteArray> { throw failure } })
        val future = CompletableFuture<ByteArray>()
        assertSame(future, observer.accountSignature { future })
        future.completeExceptionally(failure)
        assertSame(failure, assertFailsWith<CompletionException> { future.join() }.cause)
    }
}
