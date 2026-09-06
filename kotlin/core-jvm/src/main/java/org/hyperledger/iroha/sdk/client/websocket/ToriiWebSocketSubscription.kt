package org.hyperledger.iroha.sdk.client.websocket

import java.nio.ByteBuffer
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

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
    private val executor: ScheduledExecutorService = builder.executor ?: Executors.newSingleThreadScheduledExecutor {
        Thread(it, "torii-websocket-subscription").apply { isDaemon = true }
    }
    private val ownsExecutor: Boolean = builder.executor == null
    private val initialBackoffMs: Long = builder.initialBackoffMs
    private val maxBackoffMs: Long = builder.maxBackoffMs
    private val observers: List<ToriiWebSocketObserver> = builder.observers.toList()
    private val lock = Any()
    private var started = false
    private var closed = false
    private var nextBackoffMs = initialBackoffMs
    private var scheduled: PendingOpen? = null
    private var current: Attempt? = null

    /** Start the subscription once; a closed subscription cannot be restarted. */
    fun start(): ToriiWebSocketSubscription {
        synchronized(lock) {
            check(!started && !closed) { "subscription is already started or closed" }
            started = true
        }
        scheduleReconnect(0, ToriiWebSocketObserver.ReconnectReason.INITIAL)
        return this
    }

    val isRunning: Boolean get() = synchronized(lock) { started && !closed }

    override fun close() {
        val task: ScheduledFuture<*>?
        val session: ToriiWebSocketSession?
        synchronized(lock) {
            if (closed) return
            closed = true
            task = scheduled?.future
            scheduled = null
            val attempt = current
            current = null
            session = attempt?.session?.takeUnless { attempt.disposed }
            if (attempt != null) {
                attempt.terminal = true
                if (session != null) attempt.disposed = true
            }
        }
        task?.cancel(false)
        try { session?.close(1000, "client_shutdown") }
        finally { if (ownsExecutor) executor.shutdownNow() }
    }

    private fun scheduleReconnect(delayMs: Long, reason: ToriiWebSocketObserver.ReconnectReason) {
        val pending = PendingOpen()
        synchronized(lock) {
            if (closed || scheduled != null) return
            scheduled = pending
        }
        try {
            for (observer in observers) observer.onReconnectScheduled(Duration.ofMillis(delayMs), reason)
            val future = executor.schedule({ openSession(pending) }, delayMs, TimeUnit.MILLISECONDS)
            synchronized(lock) {
                pending.future = future
                if (closed || scheduled !== pending) future.cancel(false)
            }
        } catch (error: RuntimeException) {
            val active = synchronized(lock) {
                if (scheduled === pending) scheduled = null
                !closed
            }
            if (active) throw error
        }
    }

    private fun openSession(pending: PendingOpen) {
        val attempt = synchronized(lock) {
            if (closed || scheduled !== pending) return
            // Clear scheduling admission before calling the opener: it may fail synchronously.
            scheduled = null
            Attempt().also { current = it }
        }
        try { attach(attempt, opener.open(ManagedListener(attempt))) }
        catch (error: RuntimeException) { terminate(attempt, null, error, null) }
    }

    private fun attach(attempt: Attempt, session: ToriiWebSocketSession): Boolean {
        var dispose = false
        val active = synchronized(lock) {
            if (attempt.session == null) attempt.session = session
            val same = attempt.session === session
            val accepted = same && !closed && current === attempt && !attempt.terminal
            if (!accepted && (!same || !attempt.disposed)) {
                if (same) attempt.disposed = true
                dispose = true
            }
            accepted
        }
        // A late opener return owns a real pending handshake even when isOpen is still false.
        if (dispose) session.close(1000, "subscription no longer active")
        return active
    }

    private fun terminate(
        attempt: Attempt,
        session: ToriiWebSocketSession?,
        error: Throwable?,
        close: Pair<Int, String>?,
    ) {
        val target: ToriiWebSocketSession
        val delay: Long
        synchronized(lock) {
            if (closed || current !== attempt || attempt.terminal) return
            attempt.terminal = true
            current = null
            if (session != null) { attempt.session = session; attempt.disposed = true }
            target = session ?: attempt.session ?: NullSession
            delay = nextBackoffMs
            nextBackoffMs = if (nextBackoffMs >= maxBackoffMs / 2) maxBackoffMs else nextBackoffMs * 2
        }
        try {
            if (error != null) {
                delegate.onError(target, error)
                for (observer in observers) observer.onSessionFailure(error)
            } else {
                delegate.onClose(target, requireNotNull(close).first, close.second)
                for (observer in observers) observer.onSessionClosed()
            }
        } finally {
            scheduleReconnect(delay, if (error != null) ToriiWebSocketObserver.ReconnectReason.SESSION_FAILURE
                else ToriiWebSocketObserver.ReconnectReason.SESSION_CLOSED)
        }
    }

    private inner class ManagedListener(private val attempt: Attempt) : ToriiWebSocketListener {
        private fun isCurrent(): Boolean = synchronized(lock) { !closed && current === attempt && !attempt.terminal }
        override fun onOpen(session: ToriiWebSocketSession) {
            if (!attach(attempt, session)) return
            synchronized(lock) {
                if (closed || current !== attempt || attempt.terminal || attempt.opened) return
                attempt.opened = true
                nextBackoffMs = initialBackoffMs
            }
            delegate.onOpen(session)
            for (observer in observers) observer.onSessionOpened()
        }
        override fun onText(session: ToriiWebSocketSession, data: String) { if (isCurrent()) delegate.onText(session, data) }
        override fun onBinary(session: ToriiWebSocketSession, data: ByteBuffer) { if (isCurrent()) delegate.onBinary(session, data) }
        override fun onClose(session: ToriiWebSocketSession, statusCode: Int, reason: String) {
            terminate(attempt, session, null, statusCode to reason)
        }
        override fun onError(session: ToriiWebSocketSession, error: Throwable) { terminate(attempt, session, error, null) }
    }

    private class PendingOpen { var future: ScheduledFuture<*>? = null }
    private class Attempt {
        var session: ToriiWebSocketSession? = null
        var terminal = false
        var opened = false
        var disposed = false
    }

    private object NullSession : ToriiWebSocketSession {
        override fun sendText(data: String): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))
        override fun sendBinary(data: ByteBuffer): CompletableFuture<Void> = failedFuture(IllegalStateException("session not open"))

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
        internal var initialBackoffMs = 1_000L
        internal var maxBackoffMs = 30_000L
        internal val observers = ArrayList<ToriiWebSocketObserver>()

        fun setExecutor(executor: ScheduledExecutorService): Builder { this.executor = executor; return this }
        fun setInitialBackoff(duration: Duration?): Builder { if (duration != null) initialBackoffMs = maxOf(0L, duration.toMillis()); return this }
        fun setMaxBackoff(duration: Duration?): Builder { if (duration != null) maxBackoffMs = maxOf(0L, duration.toMillis()); return this }
        fun addObserver(observer: ToriiWebSocketObserver): Builder { observers.add(observer); return this }
        fun observers(values: List<ToriiWebSocketObserver>?): Builder { observers.clear(); values?.forEach { addObserver(it) }; return this }
        fun build(): ToriiWebSocketSubscription {
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
