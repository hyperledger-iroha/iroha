package org.hyperledger.iroha.sdk.client.websocket

import java.nio.ByteBuffer
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Subscription ownership includes pending handshakes and every late opener return. */
class ToriiWebSocketSubscriptionTest {
    @Test
    fun `close cancels installed pending handshake and borrows scheduler`() {
        val executor = Executors.newSingleThreadScheduledExecutor()
        try {
            val pending = Session()
            val entered = CountDownLatch(1)
            val subscription = ToriiWebSocketSubscription.builder({
                entered.countDown(); pending
            }, object : ToriiWebSocketListener {}).setExecutor(executor).build().start()
            assertTrue(entered.await(2, TimeUnit.SECONDS))
            executor.submit {}.get(2, TimeUnit.SECONDS) // Opener returned and attachment completed.
            assertFalse(pending.isOpen)
            subscription.close()
            subscription.close()
            assertEquals(1, pending.closes.get())
            assertFalse(subscription.isRunning)
            assertFalse(executor.isShutdown)
            assertEquals("borrowed", executor.submit<String> { "borrowed" }.get())
        } finally { executor.shutdownNow() }
    }

    @Test
    fun `late opener and late onOpen after close dispose session once without callback`() {
        val executor = Executors.newSingleThreadScheduledExecutor()
        val release = CountDownLatch(1)
        try {
            val entered = CountDownLatch(1)
            val pending = Session()
            val opens = AtomicInteger()
            val errors = AtomicInteger()
            val subscription = ToriiWebSocketSubscription.builder({ listener ->
                entered.countDown()
                check(release.await(5, TimeUnit.SECONDS))
                listener.onOpen(pending)
                listener.onError(pending, IllegalStateException("late callback"))
                pending
            }, object : ToriiWebSocketListener {
                override fun onOpen(session: ToriiWebSocketSession) { opens.incrementAndGet() }
                override fun onError(session: ToriiWebSocketSession, error: Throwable) { errors.incrementAndGet() }
            }).setExecutor(executor).build().start()
            assertTrue(entered.await(2, TimeUnit.SECONDS))
            subscription.close()
            release.countDown()
            executor.submit {}.get(2, TimeUnit.SECONDS)
            assertEquals(1, pending.closes.get())
            assertEquals(0, opens.get())
            assertEquals(0, errors.get())
            assertFalse(subscription.isRunning)
        } finally { release.countDown(); executor.shutdownNow() }
    }

    @Test
    fun `onOpen can synchronously close subscription before opener returns`() {
        val pending = Session()
        val finished = CompletableFuture<Void>()
        lateinit var subscription: ToriiWebSocketSubscription
        subscription = ToriiWebSocketSubscription.builder({ listener ->
            listener.onOpen(pending)
            pending
        }, object : ToriiWebSocketListener {
            override fun onOpen(session: ToriiWebSocketSession) {
                subscription.close()
                finished.complete(null)
            }
        }).build()
        subscription.start()
        finished.get(2, TimeUnit.SECONDS)
        assertEquals(1, pending.closes.get())
        assertFalse(subscription.isRunning)
        subscription.close()
    }

    @Test
    fun `synchronous handshake failure schedules the next attempt`() {
        val attempts = AtomicInteger()
        val errors = AtomicInteger()
        val connected = CompletableFuture<ToriiWebSocketSession>()
        val pending = Session()
        ToriiWebSocketSubscription.builder({ listener ->
            if (attempts.incrementAndGet() == 1) throw IllegalStateException("first handshake failed")
            listener.onOpen(pending)
            pending
        }, object : ToriiWebSocketListener {
            override fun onOpen(session: ToriiWebSocketSession) { connected.complete(session) }
            override fun onError(session: ToriiWebSocketSession, error: Throwable) { errors.incrementAndGet() }
        }).setInitialBackoff(Duration.ofMillis(1)).setMaxBackoff(Duration.ofMillis(2)).build().use { subscription ->
            subscription.start()
            assertSame(pending, connected.get(2, TimeUnit.SECONDS))
            assertEquals(2, attempts.get())
            assertEquals(1, errors.get())
        }
        assertEquals(1, pending.closes.get())
    }

    @Test
    fun `synchronous error callback is not lost while opener is running`() {
        val attempts = AtomicInteger()
        val connected = CompletableFuture<Void>()
        val first = Session()
        val second = Session()
        ToriiWebSocketSubscription.builder({ listener ->
            if (attempts.incrementAndGet() == 1) {
                listener.onError(first, IllegalStateException("failed before opener return"))
                first
            } else {
                listener.onOpen(second)
                second
            }
        }, object : ToriiWebSocketListener {
            override fun onOpen(session: ToriiWebSocketSession) { connected.complete(null) }
        }).setInitialBackoff(Duration.ofMillis(1)).build().use { subscription ->
            subscription.start()
            connected.get(2, TimeUnit.SECONDS)
            assertEquals(2, attempts.get())
        }
        assertEquals(1, second.closes.get())
    }

    private class Session : ToriiWebSocketSession {
        val closes = AtomicInteger()
        override fun sendText(data: String): CompletableFuture<Void> = CompletableFuture.completedFuture(null)
        override fun sendBinary(data: ByteBuffer): CompletableFuture<Void> = CompletableFuture.completedFuture(null)
        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> {
            closes.incrementAndGet(); return CompletableFuture.completedFuture(null)
        }
        override val isOpen: Boolean get() = false
        override val subprotocol: String get() = ""
    }
}
