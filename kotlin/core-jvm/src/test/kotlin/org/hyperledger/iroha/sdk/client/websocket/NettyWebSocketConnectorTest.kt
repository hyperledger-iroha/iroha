package org.hyperledger.iroha.sdk.client.websocket

import java.net.URI
import java.net.SocketTimeoutException
import java.nio.ByteBuffer
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import io.netty.channel.MultiThreadIoEventLoopGroup
import io.netty.channel.nio.NioIoHandler
import javax.net.ssl.SSLContext
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.RecordedRequest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Real upgrade, message and resource ownership tests for the sole JVM WebSocket backend. */
class NettyWebSocketConnectorTest {
    @Test
    fun `Torii attaches session before callbacks and preserves final handshake inputs`() {
        MockWebServer().use { server ->
            val peer = EchoPeer()
            server.enqueue(MockResponse().withWebSocketUpgrade(peer).setHeader("Sec-WebSocket-Protocol", "norito"))
            NettyWebSocketConnector.create().use { connector ->
                val opened = CompletableFuture<ToriiWebSocketSession>()
                val text = CompletableFuture<String>()
                val binary = CompletableFuture<ByteBuffer>()
                val closed = CompletableFuture<Int>()
                val listener = object : ToriiWebSocketListener {
                    override fun onOpen(session: ToriiWebSocketSession) {
                        try {
                            assertTrue(session.isOpen)
                            assertEquals("norito", session.subprotocol)
                            session.sendText("ready").get(2, TimeUnit.SECONDS)
                            opened.complete(session)
                        } catch (e: Throwable) { opened.completeExceptionally(e) }
                    }
                    override fun onText(session: ToriiWebSocketSession, data: String) {
                        text.complete(data)
                    }
                    override fun onBinary(session: ToriiWebSocketSession, data: ByteBuffer) {
                        binary.complete(data)
                    }
                    override fun onClose(session: ToriiWebSocketSession, statusCode: Int, reason: String) { closed.complete(statusCode) }
                }
                val client = ToriiWebSocketClient.builder().setBaseUri(server.url("/").toUri())
                    .putDefaultHeader("X-Default", "default").setWebSocketConnector(connector).build()
                client.connect("events?existing=1", ToriiWebSocketOptions(
                    mapOf("filter" to "a b"), mapOf("X-Trace-Id" to "exact-value"),
                    listOf("norito", "json"), Duration.ofSeconds(2),
                ), listener)
                val session = opened.await()
                assertEquals("ready", text.await())
                val bytes = byteArrayOf(9, 1, 2, 3, 9)
                val buffer = ByteBuffer.wrap(bytes).apply { position(1); limit(4) }
                session.sendBinary(buffer).await()
                bytes.fill(0)
                assertEquals(1, buffer.position())
                val received = binary.await()
                assertTrue(received.isReadOnly)
                val owned = ByteArray(received.remaining()).also { received.get(it) }
                assertContentEquals(byteArrayOf(1, 2, 3), owned)
                val request = assertNotNull(server.takeRequest(2, TimeUnit.SECONDS))
                assertEquals("/events?existing=1&filter=a+b", request.path)
                assertEquals("default", request.getHeader("X-Default"))
                assertEquals("exact-value", request.getHeader("X-Trace-Id"))
                assertEquals("norito,json", request.getHeader("Sec-WebSocket-Protocol"))
                session.close(1000, "done").await()
                assertEquals(1000, closed.await())
                assertFalse(session.isOpen)
                session.close(1000, "again").await()
                failure(session.sendText("late"))
            }
        }
    }

    @Test
    fun `invalid close and message bounds send no bytes`() {
        MockWebServer().use { server ->
            val peer = EchoPeer()
            server.enqueue(MockResponse().withWebSocketUpgrade(peer))
            NettyWebSocketConnector.create(8, 3).use { connector ->
                val socket = connector.connect(request(server), silent()).await()
                failure(socket.sendText("123456789"))
                failure(socket.sendText("ééééé"))
                failure(socket.sendText("\uD800"))
                failure(socket.sendText("\uDC00"))
                failure(socket.sendText("😀")) // Four UTF8 bytes exceed this three-byte queue.
                failure(socket.sendBinary(ByteBuffer.allocate(9)))
                failure(socket.sendText("1234")) // Queue bound is independent of message bound.
                failure(socket.close(1005, "reserved"))
                failure(socket.close(1000, "x".repeat(124)))
                assertTrue(socket.isOpen())
                socket.sendText("ok").await()
                assertEquals("ok", peer.messages.poll(2, TimeUnit.SECONDS))
                assertNull(peer.messages.poll(100, TimeUnit.MILLISECONDS))
                socket.close(1000, "done").await()
            }
        }
    }

    @Test
    fun `oversize incoming messages fail once without application delivery`() {
        for (binary in listOf(false)) MockWebServer().use { server ->
            val established = CompletableFuture<WebSocket>()
            server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) { established.complete(webSocket) }
            }))
            NettyWebSocketConnector.create(4).use { connector ->
                val errors = AtomicInteger()
                val delivered = AtomicInteger()
                val failed = CompletableFuture<Throwable>()
                val socket = connector.connect(request(server), object : TransportWebSocket.Listener {
                    override fun onError(socket: TransportWebSocket, error: Throwable) { errors.incrementAndGet(); failed.complete(error) }
                    override fun onText(socket: TransportWebSocket, data: String) { delivered.incrementAndGet() }
                    override fun onBinary(socket: TransportWebSocket, data: ByteBuffer) { delivered.incrementAndGet() }
                }).await()
                val sender = established.await()
                if (binary) sender.send(ByteArray(5).toByteString()) else sender.send("ééé")
                failed.await()
                connector.close()
                assertFalse(socket.isOpen())
                assertEquals(1, errors.get())
                assertEquals(0, delivered.get())
            }
        }
    }

    @Test
    fun `failed handshakes never replay and Torii delivers one failure`() {
        for (code in listOf(401, 307, 503)) MockWebServer().use { server ->
            server.enqueue(MockResponse().setResponseCode(code).setHeader("Retry-After", "0")
                .setHeader("Location", server.url("/redirect")))
            server.enqueue(MockResponse().setResponseCode(400))
            NettyWebSocketConnector.create().use { connector ->
                val errors = AtomicInteger()
                val observed = AtomicInteger()
                val failed = CompletableFuture<Throwable>()
                val client = ToriiWebSocketClient.builder().setBaseUri(server.url("/").toUri())
                    .setWebSocketConnector(connector).addObserver(object : ClientObserver {
                        override fun onFailure(request: TransportRequest, error: Throwable) { observed.incrementAndGet() }
                    }).build()
                val session = client.connect("events", null, object : ToriiWebSocketListener {
                    override fun onError(session: ToriiWebSocketSession, error: Throwable) { errors.incrementAndGet(); failed.complete(error) }
                })
                failed.await()
                assertEquals(1, errors.get())
                assertEquals(1, observed.get())
                assertFalse(session.isOpen)
                assertEquals("", session.subprotocol)
                assertEquals(1, server.requestCount, "HTTP $code must not redispatch the handshake")
            }
        }
    }

    @Test
    fun `unoffered subprotocol fails handshake without opening`() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().withWebSocketUpgrade(EchoPeer()).setHeader("Sec-WebSocket-Protocol", "other"))
            NettyWebSocketConnector.create().use { connector ->
                val opened = AtomicInteger()
                val errors = AtomicInteger()
                failure(connector.connect(request(server), object : TransportWebSocket.Listener {
                    override fun onOpen(socket: TransportWebSocket) { opened.incrementAndGet() }
                    override fun onError(socket: TransportWebSocket, error: Throwable) { errors.incrementAndGet() }
                }))
                assertEquals(0, opened.get())
                assertEquals(0, errors.get()) // Handshake failure belongs only to the future.
            }
        }
    }

    @Test
    fun `deadline cancellation and closed admission terminate handshakes`() {
        MockWebServer().use { server ->
            repeat(2) { server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE)) }
            NettyWebSocketConnector.create().use { connector ->
                assertIs<SocketTimeoutException>(failure(connector.connect(request(server, Duration.ofMillis(150)), silent())))
                val future = connector.connect(request(server, Duration.ofSeconds(10)), silent())
                assertNotNull(server.takeRequest(2, TimeUnit.SECONDS))
                assertNotNull(server.takeRequest(2, TimeUnit.SECONDS))
                assertTrue(future.cancel(false))
                assertTrue(future.isCancelled)
                connector.close()
                failure(connector.connect(request(server), silent()))
                assertEquals(2, server.requestCount)
            }
        }
    }

    @Test
    fun `closing borrowed connector leaves sibling socket and event loop usable`() {
        MockWebServer().use { server ->
            repeat(2) { server.enqueue(MockResponse().withWebSocketUpgrade(EchoPeer())) }
            val shared = MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory())
            try {
                NettyWebSocketConnector(shared, SSLContext.getDefault()).use { first ->
                    NettyWebSocketConnector(shared, SSLContext.getDefault()).use { second ->
                        val terminal = CompletableFuture<Throwable>()
                        val a = first.connect(request(server), object : TransportWebSocket.Listener {
                            override fun onError(socket: TransportWebSocket, error: Throwable) { terminal.complete(error) }
                        }).await()
                        val echo = CompletableFuture<String>()
                        val b = second.connect(request(server), object : TransportWebSocket.Listener {
                            override fun onText(socket: TransportWebSocket, data: String) { echo.complete(data) }
                        }).await()
                        first.close()
                        terminal.await()
                        assertFalse(a.isOpen())
                        assertTrue(b.isOpen())
                        b.sendText("sibling").await()
                        assertEquals("sibling", echo.await())
                        assertFalse(shared.isShuttingDown)
                        assertEquals("event loop still works", shared.submit<String> { "event loop still works" }.get(2, TimeUnit.SECONDS))
                        b.close(1000, "done").await()
                    }
                }
            } finally {
                shared.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly()
            }
        }
    }

    @Test
    fun `invalid handshake inputs fail before dispatch`() {
        MockWebServer().use { server ->
            NettyWebSocketConnector.create().use { connector ->
                val url = URI.create(server.url("/").toString().replaceFirst("http", "ws"))
                for (input in listOf(
                    TransportRequest.builder().setUri(url).setMethod("POST").build(),
                    TransportRequest.builder().setUri(url).setBody(byteArrayOf(1)).build(),
                    TransportRequest.builder().setUri(url).addHeader("Connection", "upgrade").build(),
                    TransportRequest.builder().setUri(url).addHeader("Sec-WebSocket-Protocol", "x,x").build(),
                    TransportRequest.builder().setUri(url).addHeader("Sec-WebSocket-Protocol", "bad protocol").build(),
                )) failure(connector.connect(input, silent()))
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test
    fun `ready continuation may close connector without a later open callback`() {
        MockWebServer().use { server ->
            val release = CountDownLatch(1)
            server.dispatcher = object : Dispatcher() {
                override fun dispatch(request: RecordedRequest): MockResponse {
                    check(release.await(5, TimeUnit.SECONDS))
                    return MockResponse().withWebSocketUpgrade(EchoPeer())
                }
            }
            NettyWebSocketConnector.create().use { connector ->
                val opens = CountDownLatch(1)
                val failure = CompletableFuture<Throwable>()
                val future = connector.connect(request(server), object : TransportWebSocket.Listener {
                    override fun onOpen(socket: TransportWebSocket) { opens.countDown() }
                    override fun onError(socket: TransportWebSocket, error: Throwable) { failure.complete(error) }
                })
                val completed = future.whenComplete { _, _ -> connector.close() }
                release.countDown()
                completed.await()
                failure.await()
                assertFalse(future.await().isOpen())
                assertFalse(opens.await(150, TimeUnit.MILLISECONDS))
            }
        }
    }

    @Test
    fun `throwing listener cannot skip closing other sockets`() {
        MockWebServer().use { server ->
            repeat(2) { server.enqueue(MockResponse().withWebSocketUpgrade(EchoPeer())) }
            NettyWebSocketConnector.create().use { connector ->
                val errors = AtomicInteger()
                val first = connector.connect(request(server), object : TransportWebSocket.Listener {
                    override fun onError(socket: TransportWebSocket, error: Throwable) {
                        errors.incrementAndGet()
                        throw IllegalStateException("application callback failed")
                    }
                }).await()
                val second = connector.connect(request(server), object : TransportWebSocket.Listener {
                    override fun onError(socket: TransportWebSocket, error: Throwable) { errors.incrementAndGet() }
                }).await()
                connector.close()
                assertFalse(first.isOpen())
                assertFalse(second.isOpen())
                assertEquals(2, errors.get())
            }
        }
    }

    private class EchoPeer : WebSocketListener() {
        val messages = LinkedBlockingQueue<String>()
        override fun onMessage(webSocket: WebSocket, text: String) { messages.add(text); webSocket.send(text) }
        override fun onMessage(webSocket: WebSocket, bytes: ByteString) { webSocket.send(bytes) }
        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) { webSocket.close(code, reason) }
    }

    private fun request(server: MockWebServer, timeout: Duration = Duration.ofSeconds(3)): TransportRequest =
        TransportRequest.builder().setUri(URI.create(server.url("/events").toString().replaceFirst("http", "ws")))
            .setTimeout(timeout).build()
    private fun silent() = object : TransportWebSocket.Listener {}
    private fun <T> CompletableFuture<T>.await(): T = get(5, TimeUnit.SECONDS)
    private fun failure(future: CompletableFuture<*>): Throwable =
        assertFailsWith<ExecutionException> { future.get(5, TimeUnit.SECONDS) }.cause!!
}
