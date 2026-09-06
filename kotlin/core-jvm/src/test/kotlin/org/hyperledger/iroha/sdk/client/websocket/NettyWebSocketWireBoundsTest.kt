package org.hyperledger.iroha.sdk.client.websocket

import io.netty.channel.MultiThreadIoEventLoopGroup
import io.netty.channel.nio.NioIoHandler
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.ServerSocket
import java.net.Socket
import java.net.URI
import java.nio.ByteBuffer
import java.security.MessageDigest
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import javax.net.ssl.SSLContext
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.ByteString
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Raw frame fixtures distinguish pre-payload wire rejection from application-only limits. */
class NettyWebSocketWireBoundsTest {
    @Test
    fun `declared oversize frame fails before peer supplies its body`() {
        RawPeer(byteArrayOf(0x82.toByte(), 126, 0x10, 0)).use { peer ->
            NettyWebSocketConnector.create(128).use { connector ->
                val error = CompletableFuture<Throwable>()
                connector.connect(peer.request(), failures(error)).await()
                val reason = error.await()
                assertContains(causes(reason), "length")
                assertFalse(peer.released) // No payload or EOF was supplied to force this failure.
            }
        }
    }

    @Test
    fun `fragment aggregation enforces total message limit`() {
        RawPeer(byteArrayOf(0x01, 5) + "hello".toByteArray() + byteArrayOf(0x80.toByte(), 4) + "more".toByteArray()).use { peer ->
            NettyWebSocketConnector.create(8).use { connector ->
                val error = CompletableFuture<Throwable>()
                val delivered = AtomicInteger()
                connector.connect(peer.request(), object : TransportWebSocket.Listener {
                    override fun onText(socket: TransportWebSocket, data: String) { delivered.incrementAndGet() }
                    override fun onError(socket: TransportWebSocket, cause: Throwable) { error.complete(cause) }
                }).await()
                assertContains(causes(error.await()), "exceed")
                assertEquals(0, delivered.get())
            }
        }
    }

    @Test
    fun `valid fragments deliver one complete message and protocol ping receives pong`() {
        val frames = byteArrayOf(0x01, 3) + "hel".toByteArray() + byteArrayOf(0x89.toByte(), 1, 7) +
            byteArrayOf(0x80.toByte(), 2) + "lo".toByteArray()
        RawPeer(frames, readPong = true).use { peer ->
            NettyWebSocketConnector.create(128).use { connector ->
                val delivered = CompletableFuture<String>()
                connector.connect(peer.request(), object : TransportWebSocket.Listener {
                    override fun onText(socket: TransportWebSocket, data: String) { delivered.complete(data) }
                }).await()
                assertEquals("hello", delivered.await())
                assertContentEquals(byteArrayOf(7), peer.pong.await())
            }
        }
    }

    @Test
    fun `malformed text never reaches the application`() {
        RawPeer(byteArrayOf(0x81.toByte(), 2, 0xc3.toByte(), 0x28)).use { peer ->
            NettyWebSocketConnector.create().use { connector ->
                val error = CompletableFuture<Throwable>()
                val delivered = AtomicInteger()
                connector.connect(peer.request(), object : TransportWebSocket.Listener {
                    override fun onText(socket: TransportWebSocket, data: String) { delivered.incrementAndGet() }
                    override fun onError(socket: TransportWebSocket, cause: Throwable) { error.complete(cause) }
                }).await()
                assertContains(causes(error.await()).lowercase(), "utf")
                assertEquals(0, delivered.get())
            }
        }
    }

    @Test
    fun `send admission reserves owned bytes before blocked event loop enqueue`() {
        val group = MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory())
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                    override fun onMessage(webSocket: WebSocket, bytes: ByteString) { webSocket.send(bytes) }
                }))
                NettyWebSocketConnector(group, SSLContext.getDefault(), 8, 3).use { connector ->
                    val echo = CompletableFuture<ByteBuffer>()
                    val socket = connector.connect(request(server.url("/ws").toString().replaceFirst("http", "ws")), object : TransportWebSocket.Listener {
                        override fun onBinary(socket: TransportWebSocket, data: ByteBuffer) { echo.complete(data) }
                    }).await()
                    val entered = CountDownLatch(1)
                    val release = CountDownLatch(1)
                    group.execute { entered.countDown(); check(release.await(5, TimeUnit.SECONDS)) }
                    assertTrue(entered.await(2, TimeUnit.SECONDS))
                    try {
                        val source = byteArrayOf(1, 2, 3)
                        socket.sendBinary(ByteBuffer.wrap(source)).await()
                        source.fill(9)
                        assertFailsWith<ExecutionException> { socket.sendText("x").await() }
                    } finally { release.countDown() }
                    val received = echo.await()
                    assertTrue(received.isReadOnly)
                    assertContentEquals(byteArrayOf(1, 2, 3), ByteArray(received.remaining()).also { received.get(it) })
                }
            }
        } finally { group.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly() }
    }

    @Test
    fun `empty messages cannot bypass pending-write bound`() {
        val group = MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory())
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {}))
                NettyWebSocketConnector(group, SSLContext.getDefault()).use { connector ->
                    val socket = connector.connect(request(server.url("/ws").toString().replaceFirst("http", "ws")),
                        object : TransportWebSocket.Listener {}).await()
                    val entered = CountDownLatch(1)
                    val release = CountDownLatch(1)
                    group.execute { entered.countDown(); check(release.await(5, TimeUnit.SECONDS)) }
                    assertTrue(entered.await(2, TimeUnit.SECONDS))
                    try {
                        repeat(1024) { socket.sendText("").await() }
                        val failure = assertFailsWith<ExecutionException> { socket.sendText("").await() }
                        assertContains(failure.cause!!.message.orEmpty(), "queued-message limit")
                    } finally { release.countDown() }
                }
            }
        } finally { group.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly() }
    }

    @Test
    fun `callback may send while connector closes and borrowed loop shuts down without lock inversion`() {
        val group = MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory())
        val closer = Executors.newSingleThreadExecutor()
        val release = CountDownLatch(1)
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                    override fun onMessage(webSocket: WebSocket, text: String) { webSocket.send(text) }
                }))
                NettyWebSocketConnector(group, SSLContext.getDefault()).use { connector ->
                    val entered = CountDownLatch(1)
                    val finished = CompletableFuture<Void>()
                    val socket = connector.connect(request(server.url("/ws").toString().replaceFirst("http", "ws")),
                        object : TransportWebSocket.Listener {
                            override fun onText(socket: TransportWebSocket, data: String) {
                                entered.countDown()
                                try {
                                    check(release.await(5, TimeUnit.SECONDS))
                                    assertFailsWith<ExecutionException> { socket.sendText("after shutdown").await() }
                                    finished.complete(null)
                                } catch (error: Throwable) { finished.completeExceptionally(error) }
                            }
                        }).await()
                    socket.sendText("callback").await()
                    assertTrue(entered.await(2, TimeUnit.SECONDS))
                    group.shutdownGracefully(0, 2, TimeUnit.SECONDS)
                    val closing = closer.submit { connector.close() }
                    val until = System.nanoTime() + TimeUnit.SECONDS.toNanos(2)
                    while (socket.isOpen() && System.nanoTime() < until) Thread.sleep(1)
                    assertFalse(socket.isOpen())
                    release.countDown()
                    finished.await()
                    closing.get(2, TimeUnit.SECONDS)
                }
            }
        } finally {
            release.countDown()
            closer.shutdownNow()
            group.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly()
        }
    }

    @Test
    fun `supplementary unicode uses its exact UTF8 bound`() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) { webSocket.send(text) }
            }))
            NettyWebSocketConnector.create(4, 8).use { connector ->
                val echo = CompletableFuture<String>()
                val socket = connector.connect(request(server.url("/ws").toString().replaceFirst("http", "ws")),
                    object : TransportWebSocket.Listener {
                        override fun onText(socket: TransportWebSocket, data: String) { echo.complete(data) }
                    }).await()
                socket.sendText("😀").await()
                assertEquals("😀", echo.await())
                assertFailsWith<ExecutionException> { socket.sendText("😀a").await() }
            }
        }
    }

    private class RawPeer(private val bytes: ByteArray, private val readPong: Boolean = false) : AutoCloseable {
        private val server = ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress())
        private val release = CountDownLatch(1)
        @Volatile var released = false
            private set
        @Volatile private var socket: Socket? = null
        val pong = CompletableFuture<ByteArray>()
        private val worker = Thread({
            try {
                server.accept().use { client ->
                    socket = client
                    client.soTimeout = 5000
                    val reader = BufferedReader(InputStreamReader(client.getInputStream(), Charsets.US_ASCII))
                    var key = ""
                    while (true) {
                        val line = reader.readLine() ?: error("handshake ended")
                        if (line.isEmpty()) break
                        if (line.startsWith("Sec-WebSocket-Key:", true)) key = line.substringAfter(':').trim()
                    }
                    check(key.isNotEmpty())
                    val accept = Base64.getEncoder().encodeToString(MessageDigest.getInstance("SHA-1")
                        .digest((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").toByteArray(Charsets.US_ASCII)))
                    client.getOutputStream().apply {
                        write(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: $accept\r\n\r\n").toByteArray(Charsets.US_ASCII))
                        write(bytes)
                        flush()
                    }
                    if (readPong) {
                        val input = client.getInputStream()
                        check(input.read() == 0x8a)
                        val size = input.read()
                        check(size and 0x80 != 0 && size and 0x7f <= 125)
                        val mask = ByteArray(4) { input.read().also { check(it >= 0) }.toByte() }
                        val payload = ByteArray(size and 0x7f) { i -> (input.read() xor mask[i % 4].toInt()).toByte() }
                        pong.complete(payload)
                    }
                    release.await(5, TimeUnit.SECONDS)
                }
            } catch (error: Exception) { pong.completeExceptionally(error) }
        }, "websocket-wire-fixture").apply { isDaemon = true; start() }
        fun request(): TransportRequest = request("ws://localhost:${server.localPort}/events")
        override fun close() {
            released = true
            release.countDown()
            socket?.close()
            server.close()
            worker.join(2000)
        }
    }

    companion object {
        private fun request(uri: String) = TransportRequest.builder().setUri(URI.create(uri)).setTimeout(Duration.ofSeconds(3)).build()
        private fun <T> CompletableFuture<T>.await(): T = get(5, TimeUnit.SECONDS)
        private fun failures(error: CompletableFuture<Throwable>) = object : TransportWebSocket.Listener {
            override fun onError(socket: TransportWebSocket, cause: Throwable) { error.complete(cause) }
        }
        private fun causes(error: Throwable): String = generateSequence(error) { it.cause }.joinToString(" ") { it.toString() }
    }
}
