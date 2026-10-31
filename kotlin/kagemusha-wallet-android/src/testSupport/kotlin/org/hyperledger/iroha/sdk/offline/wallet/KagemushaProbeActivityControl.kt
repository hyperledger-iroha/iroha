// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executor

/** Observation lifetime owned by one probe wallet, including partial registration cleanup. */
internal interface KagemushaProbeActivityObservation {
    /** Subscribe before sampling; emit the initial complete pair before returning. */
    fun start(changed: (Boolean, Boolean) -> Unit, failed: (Throwable) -> Unit)
    fun detach()
}

/**
 * Test-harness control only. The serial executor is independent of blocking monetary commands.
 * Every closure captures one exact wallet owner; retired generations can never target a reopen.
 */
internal class KagemushaProbeActivityControl(
    private val generation: Long,
    private val executor: Executor,
    private val observation: KagemushaProbeActivityObservation,
    private val publish: (Boolean, Boolean) -> Unit,
    private val closeOwner: (retry: Boolean) -> Unit,
    private val shutdown: () -> Unit,
) : AutoCloseable {
    private val gate = Any()
    private var started = false
    private var accepting = true
    private var initial: CompletableFuture<Unit>? = null
    private var failure: Throwable? = null
    private var controlsDrained = false
    private var closeAttempted = false
    private var ownerReleased = false
    @Volatile var released = false
        private set

    /** Caller is off-main; initial Native publication is acknowledged before commands/open output. */
    fun start() {
        synchronized(gate) { check(!started && accepting); started = true }
        try {
            observation.start({ foreground, charging -> changed(generation, foreground, charging) }, ::failed)
            val acknowledgment = synchronized(gate) { initial }
                ?: throw IllegalStateException("activity-initial-sample-missing")
            await(acknowledgment)
            checkHealthy()
        } catch (problem: Throwable) {
            failed(problem)
            throw problem
        }
    }

    /** Capture a complete immutable pair; no Native call or wait occurs on the callback thread. */
    fun changed(sourceGeneration: Long, foreground: Boolean, charging: Boolean) {
        synchronized(gate) {
            if (sourceGeneration != generation || !accepting || failure != null) return
            val acknowledgment = CompletableFuture<Unit>()
            if (initial == null) initial = acknowledgment
            try {
                executor.execute {
                    val admitted = synchronized(gate) { accepting && failure == null }
                    if (!admitted) {
                        acknowledgment.completeExceptionally(IllegalStateException("activity-retired"))
                    } else {
                        try {
                            publish(foreground, charging)
                            acknowledgment.complete(Unit)
                        } catch (problem: Throwable) {
                            failed(problem)
                            acknowledgment.completeExceptionally(problem)
                        }
                    }
                }
            } catch (problem: Throwable) {
                failed(problem)
                acknowledgment.completeExceptionally(problem)
            }
        }
    }

    /** AndroidX may log and swallow callback exceptions; retain them explicitly for the driver. */
    fun failed(problem: Throwable) {
        synchronized(gate) {
            if (failure == null) failure = problem
            else if (failure !== problem && failure!!.suppressed.none { it === problem }) failure!!.addSuppressed(problem)
            initial?.completeExceptionally(failure!!)
        }
    }

    fun checkHealthy() {
        synchronized(gate) {
            failure?.let { throw it }
            check(accepting && started && initial?.isDone == true) { "activity-not-ready" }
        }
    }

    /** Invalidate first; even a queued initial sample must wake its waiter on retirement. */
    fun retire() {
        synchronized(gate) {
            accepting = false
            initial?.completeExceptionally(IllegalStateException("activity-retired"))
        }
    }

    /**
     * Retire callbacks and drain the final inactivity attempt before Native close. Publication
     * failure cannot prevent close after the fence; failed detach retains observation ownership.
     * Cleanup reports sticky failures even after release. A failed close retains this exact owner.
     * Only the command/cleanup owner calls close; observation callbacks never call it.
     */
    override fun close() {
        retire()
        if (!released) {
            var detached = false
            try { observation.detach(); detached = true } catch (problem: Throwable) { failed(problem) }
            if (!controlsDrained) {
                val acknowledgment = CompletableFuture<Unit>()
                try {
                    executor.execute {
                        try { publish(false, false); acknowledgment.complete(Unit) }
                        catch (problem: Throwable) { acknowledgment.completeExceptionally(problem) }
                    }
                    // Interrupted callers still fence admitted controls before releasing custody.
                    try { awaitDrain(acknowledgment) } finally { controlsDrained = true }
                } catch (problem: Throwable) { failed(problem) }
                // If submission failed, no fence proves an older in-flight control has returned.
            }
            if (controlsDrained && !ownerReleased) {
                val retry = closeAttempted
                closeAttempted = true
                try { closeOwner(retry); ownerReleased = true } catch (problem: Throwable) { failed(problem) }
            }
            if (ownerReleased && detached) {
                try { shutdown(); released = true } catch (problem: Throwable) { failed(problem) }
            }
        }
        synchronized(gate) { failure?.let { throw it } }
    }

    private fun await(value: CompletableFuture<Unit>) {
        try { value.get() }
        catch (problem: ExecutionException) { throw problem.cause ?: problem }
        catch (problem: InterruptedException) { Thread.currentThread().interrupt(); throw problem }
    }

    private fun awaitDrain(value: CompletableFuture<Unit>) {
        var interrupted = false
        try {
            while (true) {
                try { value.get(); return }
                catch (_: InterruptedException) { interrupted = true }
                catch (problem: ExecutionException) { throw problem.cause ?: problem }
            }
        } finally { if (interrupted) Thread.currentThread().interrupt() }
    }
}
