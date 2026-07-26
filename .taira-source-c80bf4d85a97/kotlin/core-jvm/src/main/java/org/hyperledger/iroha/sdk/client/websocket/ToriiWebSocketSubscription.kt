package org.hyperledger.iroha.sdk.client.websocket

import java.nio.ByteBuffer
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

/**
 * Convenience helper that keeps a Torii WebSocket session alive by reconnecting when the underlying
 * connection drops. Reconnection delays follow an exponential backoff policy.
 */
class ToriiWebSocketSubscription private constructor(builder: Builder) : AutoCloseable {

    /** Factory used to open WebSocket sessions (allows dependency injection in tests). */
    fun interface SessionOpener {
        fun open(listener: ToriiWebSocketListener): ToriiWebSocketSession
    }

    private val opener: SessionOpener = builder.opener
    private val delegate: ToriiWebSocketListener = builder.listener
    private val executor: ScheduledExecutorService = builder.executor!!
    private val ownsExecutor: Boolean = builder.ownsExecutor
    private val initialBackoffMs: Long = builder.initialBackoffMs
    private val maxBackoffMs: Long = builder.maxBackoffMs
    private val started = AtomicBoolean(false)
    private val closed = AtomicBoolean(false)
    private val nextBackoffMs = AtomicLong(builder.initialBackoffMs)
    private val sessionGeneration = AtomicLong(0)
    private val currentSession = AtomicReference<ToriiWebSocketSession?>(null)
    private val scheduledTask = AtomicReference<ScheduledFuture<*>?>(null)
    private val observers: List<ToriiWebSocketObserver> = builder.observers.toList()

    @Synchronized
    fun start(): ToriiWebSocketSubscription {
        check(started.compareAndSet(false, true)) { "subscription already started" }
        scheduleReconnect(0, ToriiWebSocketObserver.ReconnectReason.INITIAL)
        return this
    }

    val isRunning: Boolean get() = started.get() && !closed.get()

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        scheduledTask.getAndSet(null)?.cancel(true)
        val session = currentSession.getAndSet(null)
        if (session != null && session.isOpen) session.close(1000, "client_shutdown")
        if (ownsExecutor) executor.shutdownNow()
    }

    private fun scheduleReconnect(delayMs: Long, reason: ToriiWebSocketObserver.ReconnectReason) {
        if (closed.get()) return
        val existing = scheduledTask.get()
        if (existing != null && !existing.isDone && !existing.isCancelled) return
        val clampedDelay = maxOf(0L, delayMs)
        notifyReconnectScheduled(clampedDelay, reason)
        scheduledTask.set(executor.schedule(::openSession, clampedDelay, TimeUnit.MILLISECONDS))
    }

    private fun notifyReconnectScheduled(delayMs: Long, reason: ToriiWebSocketObserver.ReconnectReason) {
        if (observers.isEmpty()) return
        val duration = Duration.ofMillis(maxOf(0L, delayMs))
        for (observer in observers) observer.onReconnectScheduled(duration, reason)
    }

    private fun notifySessionOpened() { for (observer in observers) observer.onSessionOpened() }
    private fun notifySessionClosed() { for (observer in observers) observer.onSessionClosed() }
    private fun notifySessionFailure(error: Throwable) { for (observer in observers) observer.onSessionFailure(error) }

    private fun openSession() {
        if (closed.get()) return
        val generation = sessionGeneration.incrementAndGet()
        val listener = ManagedListener(generation)
        try {
            val session = opener.open(listener)
            currentSession.set(session)
        } catch (ex: RuntimeException) {
            handleFailure(generation, null, ex)
        }
    }

    private fun handleFailure(generation: Long, session: ToriiWebSocketSession?, error: Throwable) {
        if (closed.get()) return
        if (generation != sessionGeneration.get()) return
        val targetSession = session ?: currentSession.get()
        delegate.onError(targetSession ?: NullSession, error)
        notifySessionFailure(error)
        scheduleReconnect(pickBackoffDelay(), ToriiWebSocketObserver.ReconnectReason.SESSION_FAILURE)
    }

    private fun pickBackoffDelay(): Long = nextBackoffMs.getAndUpdate(::computeNextBackoff)

    private fun computeNextBackoff(current: Long): Long {
        val doubled = minOf(maxBackoffMs, current * 2)
        return if (doubled <= 0) maxBackoffMs else doubled
    }

    private inner class ManagedListener(private val generation: Long) : ToriiWebSocketListener {
        private fun isCurrent(): Boolean = generation == sessionGeneration.get()
        override fun onOpen(session: ToriiWebSocketSession) {
            if (!isCurrent()) return
            nextBackoffMs.set(initialBackoffMs)
            delegate.onOpen(session)
            notifySessionOpened()
        }
        override fun onText(session: ToriiWebSocketSession, data: CharSequence, last: Boolean) { if (isCurrent()) delegate.onText(session, data, last) }
        override fun onBinary(session: ToriiWebSocketSession, data: ByteBuffer, last: Boolean) { if (isCurrent()) delegate.onBinary(session, data, last) }
        override fun onPing(session: ToriiWebSocketSession, message: ByteBuffer) { if (isCurrent()) delegate.onPing(session, message) }
        override fun onPong(session: ToriiWebSocketSession, message: ByteBuffer) { if (isCurrent()) delegate.onPong(session, message) }
        override fun onClose(session: ToriiWebSocketSession, statusCode: Int, reason: String) {
            if (!isCurrent()) return
            delegate.onClose(session, statusCode, reason)
            notifySessionClosed()
            scheduleReconnect(pickBackoffDelay(), ToriiWebSocketObserver.ReconnectReason.SESSION_CLOSED)
        }
        override fun onError(session: ToriiWebSocketSession, error: Throwable) { handleFailure(generation, session, error) }
    }

    private object NullSession : ToriiWebSocketSession {
        override fun sendText(data: CharSequence, last: Boolean): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))
        override fun sendBinary(data: ByteBuffer, last: Boolean): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))
        override fun sendPing(message: ByteBuffer): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))
        override fun sendPong(message: ByteBuffer): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))

        private fun <T> failedFuture(ex: Throwable): CompletableFuture<T> =
            CompletableFuture<T>().also { it.completeExceptionally(ex) }
        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> = CompletableFuture.completedFuture(null)
        override val isOpen: Boolean get() = false
        override val subprotocol: String? get() = ""
    }

    class Builder internal constructor(
        internal val listener: ToriiWebSocketListener,
        internal val opener: SessionOpener
    ) {
        internal var executor: ScheduledExecutorService? = null
        internal var ownsExecutor = false
        internal var initialBackoffMs = 1_000L
        internal var maxBackoffMs = 30_000L
        internal val observers = ArrayList<ToriiWebSocketObserver>()

        fun setExecutor(executor: ScheduledExecutorService): Builder { this.executor = executor; ownsExecutor = false; return this }
        fun setInitialBackoff(duration: Duration?): Builder { if (duration != null) initialBackoffMs = maxOf(0L, duration.toMillis()); return this }
        fun setMaxBackoff(duration: Duration?): Builder { if (duration != null) maxBackoffMs = maxOf(0L, duration.toMillis()); return this }
        fun addObserver(observer: ToriiWebSocketObserver): Builder { observers.add(observer); return this }
        fun observers(values: List<ToriiWebSocketObserver>?): Builder { observers.clear(); values?.forEach { addObserver(it) }; return this }
        fun build(): ToriiWebSocketSubscription {
            if (executor == null) {
                executor = Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "torii-websocket-subscription").apply { isDaemon = true } }
                ownsExecutor = true
            }
            if (initialBackoffMs == 0L) initialBackoffMs = 1_000L
            if (maxBackoffMs < initialBackoffMs) maxBackoffMs = initialBackoffMs
            return ToriiWebSocketSubscription(this)
        }
    }

    companion object {
        @JvmStatic
        fun builder(client: ToriiWebSocketClient, path: String, options: ToriiWebSocketOptions?, listener: ToriiWebSocketListener): Builder {
            val resolved = options ?: ToriiWebSocketOptions.defaultOptions()
            return Builder(listener) { delegate -> client.connect(path, resolved, delegate) }
        }

        @JvmStatic
        fun builder(opener: SessionOpener, listener: ToriiWebSocketListener): Builder = Builder(listener, opener)
    }
}
