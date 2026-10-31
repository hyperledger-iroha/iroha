// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import java.util.concurrent.CompletableFuture
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Native's exact end observation; Ending still retains accepted Native children. */
enum class KagemushaWalletExchangeEndV1 { ENDED, ENDING, ALREADY_ENDED }

/** Local task custody only. Native's shared scheduler owns every Archive admission decision. */
internal class KagemushaWalletExchangeLifetimeV1(
    private val endNative: () -> KagemushaWalletExchangeEndV1,
) {
    private val lock = ReentrantLock()
    private val changed = lock.newCondition()
    private var ending = false
    private var ended = false
    private var accepted = 0
    private class EndAttempt {
        var done = false
        var result: KagemushaWalletExchangeEndV1? = null
        var failure: Throwable? = null
    }
    private var joining: EndAttempt? = null

    fun admit(): KagemushaWalletExchangeTaskV1 = lock.withLock {
        check(!ending) { "Logical exchange no longer admits work" }
        check(accepted < Int.MAX_VALUE) { "Logical exchange task limit" }
        accepted++
        KagemushaWalletExchangeTaskV1 { release() }
    }

    private fun release() = lock.withLock {
        check(accepted > 0)
        accepted--
        changed.signalAll()
    }

    fun released(): Boolean = lock.withLock { ended }

    /** Fence admission before enqueueing an off-UI end/join operation. */
    fun requestEnd() = lock.withLock { ending = true }

    /** Fence immediately, join accepted tasks, then call Native without holding a monitor.
     * Run off the UI thread and finish this caller's accepted task before entering.
     * Interruption is restored after the exact outcome; it never abandons accepted work.
     * A new explicit call after failure retries the same Native end closure and token. */
    fun end(): KagemushaWalletExchangeEndV1 {
        var interrupted = false
        lock.lock()
        try {
            ending = true
            if (ended) return KagemushaWalletExchangeEndV1.ALREADY_ENDED
            val current = joining
            if (current != null) {
                while (!current.done) {
                    try { changed.await() } catch (_: InterruptedException) { interrupted = true }
                }
                current.failure?.let { throw it }
                return checkNotNull(current.result)
            }
            val currentAttempt = EndAttempt()
            joining = currentAttempt
            while (accepted != 0) {
                try { changed.await() } catch (_: InterruptedException) { interrupted = true }
            }
            lock.unlock()
            val answer: KagemushaWalletExchangeEndV1
            try {
                answer = endNative()
            } catch (error: Throwable) {
                lock.lock()
                currentAttempt.failure = error
                currentAttempt.done = true
                joining = null
                changed.signalAll()
                throw error
            }
            lock.lock()
            currentAttempt.result = answer
            currentAttempt.done = true
            ended = answer != KagemushaWalletExchangeEndV1.ENDING
            joining = null
            changed.signalAll()
            return answer
        } finally {
            lock.unlock()
            if (interrupted) Thread.currentThread().interrupt()
        }
    }
}

/** An accepted pre-enqueue task. Finishing is idempotent and never abandons an active call. */
internal class KagemushaWalletExchangeTaskV1(private val release: () -> Unit) : Closeable {
    private val lock = Any()
    private var finished = false
    private var released = false
    private var active = 0

    fun <T> useCall(action: () -> T): T {
        enter()
        try { return action() }
        finally { leave() }
    }

    /** Retain one parent call until the future settles and every entered continuation returns.
     * Cancellation closes admission for later callbacks; it cannot release an in-flight call. */
    fun <T> useFuture(action: (KagemushaWalletExchangeTaskV1) -> CompletableFuture<T>): CompletableFuture<T> {
        enter()
        val continuation = KagemushaWalletExchangeTaskV1 { leave() }
        try {
            val result = action(continuation)
            result.whenComplete { _, _ -> continuation.close() }
            return result
        } catch (failure: Throwable) {
            continuation.close()
            throw failure
        }
    }

    private fun enter() {
        synchronized(lock) {
            check(!finished) { "Accepted exchange task has finished" }
            check(active < Int.MAX_VALUE) { "Accepted exchange call limit" }
            active++
        }
    }

    private fun leave() {
        val done = synchronized(lock) {
            check(active > 0)
            active--
            (finished && active == 0 && !released).also { if (it) released = true }
        }
        if (done) release()
    }

    override fun close() {
        val done = synchronized(lock) {
            finished = true
            (active == 0 && !released).also { if (it) released = true }
        }
        if (done) release()
    }
}
