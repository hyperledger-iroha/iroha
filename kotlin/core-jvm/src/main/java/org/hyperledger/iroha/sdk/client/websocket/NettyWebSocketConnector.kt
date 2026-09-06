package org.hyperledger.iroha.sdk.client.websocket

import io.netty.bootstrap.Bootstrap
import io.netty.buffer.Unpooled
import io.netty.buffer.UnpooledByteBufAllocator
import io.netty.channel.Channel
import io.netty.channel.ChannelFactory
import io.netty.channel.ChannelHandlerContext
import io.netty.channel.ChannelInboundHandlerAdapter
import io.netty.channel.ChannelInitializer
import io.netty.channel.ChannelOption
import io.netty.channel.EventLoopGroup
import io.netty.channel.FixedRecvByteBufAllocator
import io.netty.channel.MultiThreadIoEventLoopGroup
import io.netty.channel.nio.NioIoHandler
import io.netty.channel.socket.SocketChannel
import io.netty.channel.socket.nio.NioSocketChannel
import io.netty.handler.codec.http.DefaultHttpHeaders
import io.netty.handler.codec.http.FullHttpResponse
import io.netty.handler.codec.http.HttpClientCodec
import io.netty.handler.codec.http.HttpObjectAggregator
import io.netty.handler.codec.http.websocketx.BinaryWebSocketFrame
import io.netty.handler.codec.http.websocketx.CloseWebSocketFrame
import io.netty.handler.codec.http.websocketx.PingWebSocketFrame
import io.netty.handler.codec.http.websocketx.PongWebSocketFrame
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame
import io.netty.handler.codec.http.websocketx.Utf8FrameValidator
import io.netty.handler.codec.http.websocketx.WebSocketClientHandshaker
import io.netty.handler.codec.http.websocketx.WebSocketClientHandshakerFactory
import io.netty.handler.codec.http.websocketx.WebSocketCloseStatus
import io.netty.handler.codec.http.websocketx.WebSocketFrame
import io.netty.handler.codec.http.websocketx.WebSocketFrameAggregator
import io.netty.handler.codec.http.websocketx.WebSocketVersion
import io.netty.handler.ssl.SslHandler
import io.netty.util.ReferenceCountUtil
import java.io.IOException
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.time.Duration
import java.util.ArrayDeque
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Future
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.ThreadFactory
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket

/**
 * Canonical JDK 8 WebSocket engine with one explicit TCP connection and one HTTP upgrade attempt.
 * It never redirects, retries an upgrade, or resubmits authentication headers. The immutable
 * TransportRequest is the sole source of application headers, subprotocol offers and timeout.
 * TLS uses the supplied JDK SSLContext with HTTPS hostname verification enforced unconditionally.
 *
 * The constructor borrows a NIO-compatible event-loop group and TLS context; create() owns its
 * NIO group. Every connector owns its bounded system-DNS worker pool and deadline scheduler.
 * Closing cancels only this connector's sessions and pending handshakes, including late DNS work.
 * It never shuts down a borrowed event-loop group. No native transport or runtime discovery is used.
 *
 * Public sends and callbacks contain complete messages. Incoming wire fragments are bounded by
 * the frame decoder and aggregate limit before application delivery; protocol control frames stay
 * internal. Each socket reserves outgoing bytes before copying/enqueuing them. Successful send
 * futures mean queue acceptance, not peer acknowledgement. Binary buffers are copied on send and
 * independently owned/read-only on delivery. At most 1,024 writes may be pending per socket,
 * including empty messages and protocol pong responses. Callbacks run on the I/O or terminating caller thread
 * and must not block. Closing a connector from one of its callbacks is supported.
 */
class NettyWebSocketConnector private constructor(
    private val group: EventLoopGroup,
    private val tlsContext: SSLContext,
    private val ownsGroup: Boolean,
    private val maximumMessageBytes: Int,
    private val maximumQueuedBytes: Long,
) : ToriiWebSocketClient.WebSocketConnector, AutoCloseable {
    private val lock = Any()
    private var closed = false
    private val sessions = LinkedHashSet<Session>()
    private val deadlines = ScheduledThreadPoolExecutor(1, daemonThreads("iroha-websocket-deadline"))
        .apply { removeOnCancelPolicy = true }
    private val resolver = ThreadPoolExecutor(
        2, 2, 0L, TimeUnit.MILLISECONDS, ArrayBlockingQueue<Runnable>(128),
        daemonThreads("iroha-websocket-dns"), ThreadPoolExecutor.AbortPolicy(),
    )

    init {
        require(maximumMessageBytes in 1..MAXIMUM_MESSAGE_BYTES) { "maximumMessageBytes must be between 1 and 16 MiB" }
        require(maximumQueuedBytes in 1..MAXIMUM_QUEUED_BYTES) { "maximumQueuedBytes must be between 1 and 16 MiB" }
    }

    /** Borrow explicit application NIO and JDK TLS resources. */
    @JvmOverloads
    constructor(
        group: EventLoopGroup,
        tlsContext: SSLContext,
        maximumMessageBytes: Int = DEFAULT_MAXIMUM_MESSAGE_BYTES,
        maximumQueuedBytes: Long = DEFAULT_MAXIMUM_QUEUED_BYTES,
    ) : this(group, tlsContext, false, maximumMessageBytes, maximumQueuedBytes)

    override fun connect(request: TransportRequest, listener: TransportWebSocket.Listener): CompletableFuture<TransportWebSocket> {
        val session = try { Session(request, listener) } catch (error: Exception) { return failed(error) }
        synchronized(lock) {
            if (closed) return failed(IllegalStateException("WebSocket connector is closed"))
            sessions.add(session)
        }
        session.ready.whenComplete { _, _ ->
            if (session.ready.isCancelled) session.fail(CancellationException("WebSocket handshake cancelled"))
        }
        session.start()
        return session.ready
    }

    override fun close() {
        val active = synchronized(lock) {
            if (closed) return
            closed = true
            sessions.toList().also { sessions.clear() }
        }
        for (session in active) session.fail(CancellationException("WebSocket connector closed"))
        resolver.shutdownNow()
        deadlines.shutdownNow()
        if (ownsGroup) group.shutdownGracefully(0, 5, TimeUnit.SECONDS)
    }

    private inner class Session(
        private val request: TransportRequest,
        private val listener: TransportWebSocket.Listener,
    ) : TransportWebSocket {
        val ready = CompletableFuture<TransportWebSocket>()
        private val stateLock = Any()
        private val callbackLock = Any()
        private var state = State.CONNECTING
        private var channel: Channel? = null
        private var dns: Future<*>? = null
        private var deadline: ScheduledFuture<*>? = null
        private var closeDeadline: ScheduledFuture<*>? = null
        private var opened = false
        private var selectedProtocol = ""
        private var queuedBytes = 0L
        private var queuedMessages = 0
        private val outbound = ArrayDeque<PendingWrite>()
        private var writeScheduled = false
        private var upgradeStarted = false
        private var peerClose: Pair<Int, String>? = null
        private val closedFuture = CompletableFuture<Void>()
        private val host = requireNotNull(request.uri.host) { "WebSocket URI requires a host" }
        private val secure = request.uri.scheme == "wss"
        private val port = if (request.uri.port == -1) if (secure) 443 else 80 else request.uri.port
        private val timeout = request.timeout ?: Duration.ofSeconds(10)
        private val handshaker: WebSocketClientHandshaker

        init {
            require(request.method == "GET" && request.body.isEmpty()) { "WebSocket handshake requires GET with an empty body" }
            require(request.uri.scheme in setOf("ws", "wss") && request.uri.rawFragment == null && request.uri.userInfo == null) {
                "WebSocket URI must use ws/wss without user information or fragment"
            }
            require(port in 1..65535) { "Invalid WebSocket port" }
            timeout.toNanos()
            val headers = DefaultHttpHeaders()
            val offers = ArrayList<String>()
            var headerBytes = 0L
            for ((name, values) in request.headers) {
                require(MANAGED_HEADERS.none { it.equals(name, true) }) { "WebSocket framing header is transport-owned: $name" }
                for (value in values) {
                    headerBytes += name.length.toLong() + value.length + 4
                    require(headerBytes <= MAXIMUM_HANDSHAKE_BYTES) { "WebSocket headers exceed the handshake limit" }
                    if (name.equals("Sec-WebSocket-Protocol", true)) offers.addAll(value.split(',').map { it.trim() })
                    else headers.add(name, value)
                }
            }
            require(offers.all(::isProtocolToken) && offers.toSet().size == offers.size) {
                "WebSocket subprotocol offers must be distinct non-empty HTTP tokens"
            }
            require(request.uri.toASCIIString().length <= MAXIMUM_HANDSHAKE_BYTES) { "WebSocket URI exceeds the handshake limit" }
            handshaker = WebSocketClientHandshakerFactory.newHandshaker(
                request.uri, WebSocketVersion.V13, offers.takeIf { it.isNotEmpty() }?.joinToString(","),
                false, headers, maxOf(125, maximumMessageBytes), true, false, CLOSE_TIMEOUT_MILLIS, false, false,
            )
        }

        fun start() {
            try {
                if (!timeout.isZero) {
                    val task = deadlines.schedule({
                        fail(SocketTimeoutException("WebSocket handshake timed out"), connectingOnly = true)
                    }, timeout.toNanos(), TimeUnit.NANOSECONDS)
                    synchronized(stateLock) { if (state == State.CONNECTING) deadline = task else task.cancel(false) }
                }
                val work = resolver.submit {
                    try {
                        // Resolve away from the I/O loop; the independent deadline can cancel a stuck resolver.
                        val address = InetSocketAddress(InetAddress.getByName(host), port)
                        if (synchronized(stateLock) { state != State.CONNECTING }) return@submit
                        val connecting = Bootstrap().group(group)
                            .channelFactory(ChannelFactory { NioSocketChannel() })
                            .option(ChannelOption.CONNECT_TIMEOUT_MILLIS, 0)
                            .option(ChannelOption.ALLOCATOR, UnpooledByteBufAllocator(false))
                            .option(ChannelOption.RECVBUF_ALLOCATOR, FixedRecvByteBufAllocator(8192))
                            .handler(object : ChannelInitializer<SocketChannel>() {
                                override fun initChannel(ch: SocketChannel) { configure(ch) }
                            }).connect(address)
                        if (synchronized(stateLock) { state == State.TERMINATED }) connecting.channel().close()
                        connecting.addListener { future -> if (!future.isSuccess) fail(future.cause()) }
                    } catch (error: Exception) { fail(error) }
                }
                synchronized(stateLock) { if (state == State.CONNECTING) dns = work else work.cancel(false) }
            } catch (error: Exception) { fail(error) }
        }

        private fun configure(ch: SocketChannel) {
            val accepted = synchronized(stateLock) {
                if (state != State.CONNECTING) false else { channel = ch; true }
            }
            if (!accepted) { ch.close(); return }
            val pipeline = ch.pipeline()
            if (secure) {
                val engine = tlsContext.createSSLEngine(host, port)
                engine.useClientMode = true
                engine.sslParameters = engine.sslParameters.apply { endpointIdentificationAlgorithm = "HTTPS" }
                pipeline.addLast("tls", SslHandler(engine).apply { handshakeTimeoutMillis = 0 })
            }
            pipeline.addLast("http", HttpClientCodec(4096, MAXIMUM_HANDSHAKE_BYTES, 8192))
            pipeline.addLast("http-body", HttpObjectAggregator(MAXIMUM_HANDSHAKE_BYTES))
            pipeline.addLast("utf8", Utf8FrameValidator())
            pipeline.addLast("messages", WebSocketFrameAggregator(maximumMessageBytes))
            pipeline.addLast("session", object : ChannelInboundHandlerAdapter() {
                override fun channelActive(ctx: ChannelHandlerContext) {
                    val tls = ctx.pipeline().get(SslHandler::class.java)
                    if (tls == null) beginUpgrade(ctx.channel()) else tls.handshakeFuture().addListener { future ->
                        if (future.isSuccess) beginUpgrade(ctx.channel()) else fail(future.cause())
                    }
                }
                override fun channelRead(ctx: ChannelHandlerContext, message: Any) {
                    try {
                        when (message) {
                            is FullHttpResponse -> {
                                check(!handshaker.isHandshakeComplete) { "Unexpected HTTP response after WebSocket upgrade" }
                                handshaker.finishHandshake(ctx.channel(), message)
                                onOpen()
                            }
                            is TextWebSocketFrame -> deliver(message, text = true)
                            is BinaryWebSocketFrame -> deliver(message, text = false)
                            is PingWebSocketFrame -> sendControlPong(message)
                            is PongWebSocketFrame -> Unit
                            is CloseWebSocketFrame -> receiveClose(message)
                            else -> throw IOException("Unexpected WebSocket input")
                        }
                    } catch (error: Exception) { fail(error) }
                    finally { ReferenceCountUtil.release(message) }
                }
                override fun exceptionCaught(ctx: ChannelHandlerContext, cause: Throwable) { fail(cause) }
                override fun channelInactive(ctx: ChannelHandlerContext) { inactive() }
            })
        }

        private fun beginUpgrade(ch: Channel) {
            val accepted = synchronized(stateLock) {
                if (state != State.CONNECTING || upgradeStarted) false else { upgradeStarted = true; true }
            }
            if (!accepted) { ch.close(); return }
            handshaker.handshake(ch).addListener { future -> if (!future.isSuccess) fail(future.cause()) }
        }

        private fun onOpen() = synchronized(callbackLock) {
            val accepted = synchronized(stateLock) {
                if (state != State.CONNECTING) false else {
                    state = State.OPEN
                    opened = true
                    selectedProtocol = handshaker.actualSubprotocol().orEmpty()
                    deadline?.cancel(false)
                    true
                }
            }
            if (accepted) {
                ready.complete(this)
                // A ready continuation can synchronously close the connector before onOpen dispatch.
                if (isOpen()) listener.onOpen(this)
            }
        }

        private fun deliver(message: WebSocketFrame, text: Boolean) = synchronized(callbackLock) {
            if (isOpen()) {
                val size = message.content().readableBytes()
                require(size <= maximumMessageBytes) { "WebSocket message exceeds the accepted byte limit" }
                val bytes = ByteArray(size)
                message.content().getBytes(message.content().readerIndex(), bytes)
                if (text) listener.onText(this, String(bytes, Charsets.UTF_8))
                else listener.onBinary(this, ByteBuffer.wrap(bytes).asReadOnlyBuffer())
            }
        }

        override fun sendText(data: String): CompletableFuture<Void> = sending {
            require(data.length <= maximumMessageBytes) { "WebSocket message exceeds the accepted byte limit" }
            val size = utf8Length(data)
            enqueue(size) { TextWebSocketFrame(Unpooled.wrappedBuffer(utf8(data))) }
        }

        override fun sendBinary(data: ByteBuffer): CompletableFuture<Void> = sending {
            val source = data.asReadOnlyBuffer()
            enqueue(source.remaining()) {
                val bytes = ByteArray(source.remaining()).also { source.get(it) }
                BinaryWebSocketFrame(Unpooled.wrappedBuffer(bytes))
            }
        }

        private fun enqueue(size: Int, frame: () -> WebSocketFrame) {
            synchronized(stateLock) {
                check(state == State.OPEN) { "WebSocket is not open" }
                require(size <= maximumMessageBytes) { "WebSocket message exceeds the accepted byte limit" }
                reserve(size)
                val message = try { frame() } catch (error: Exception) { unreserve(size); throw error }
                outbound.addLast(PendingWrite(message, size))
            }
            scheduleWrites()
        }

        private fun reserve(size: Int) {
            check(queuedBytes <= maximumQueuedBytes - size) { "WebSocket send exceeds the queued-byte limit" }
            check(queuedMessages < MAXIMUM_QUEUED_MESSAGES) { "WebSocket send exceeds the queued-message limit" }
            queuedBytes += size
            queuedMessages++
        }

        private fun unreserve(size: Int) { queuedBytes -= size; queuedMessages-- }

        private fun scheduleWrites() {
            val target = synchronized(stateLock) {
                if (writeScheduled || state == State.TERMINATED || outbound.isEmpty()) return
                writeScheduled = true
                requireNotNull(channel)
            }
            try { target.eventLoop().execute { drainWrites(target) } }
            catch (error: RuntimeException) { fail(error); throw error }
        }

        private fun drainWrites(target: Channel) {
            while (true) {
                val next = synchronized(stateLock) {
                    val pending = outbound.pollFirst()
                    if (pending == null) writeScheduled = false
                    pending
                } ?: return
                try {
                    target.writeAndFlush(next.frame).addListener { future ->
                        synchronized(stateLock) { next.bytes?.let(::unreserve) }
                        if (!future.isSuccess) fail(future.cause())
                        else if (next.closeAfter) target.close()
                    }
                } catch (error: RuntimeException) {
                    ReferenceCountUtil.safeRelease(next.frame)
                    synchronized(stateLock) { next.bytes?.let(::unreserve) }
                    fail(error)
                }
            }
        }

        private fun sendControlPong(ping: PingWebSocketFrame) {
            synchronized(stateLock) {
                if (state != State.OPEN) return
                val size = ping.content().readableBytes()
                reserve(size)
                outbound.addLast(PendingWrite(PongWebSocketFrame(ping.content().retainedDuplicate()), size))
            }
            scheduleWrites()
        }

        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> {
            try {
                require(WebSocketCloseStatus.isValidStatusCode(statusCode)) { "Invalid WebSocket close status: $statusCode" }
                require(reason.length <= 123 && utf8Length(reason) <= 123) { "WebSocket close reason exceeds 123 bytes" }
                synchronized(stateLock) {
                    if (state == State.CLOSING || state == State.TERMINATED) return closedFuture
                    check(state == State.OPEN) { "WebSocket is not open" }
                    state = State.CLOSING
                    outbound.addLast(PendingWrite(CloseWebSocketFrame(statusCode, reason), null))
                }
                scheduleCloseDeadline()
                scheduleWrites()
            } catch (error: Exception) { return failed(error) }
            return closedFuture
        }

        private fun scheduleCloseDeadline() {
            val task = deadlines.schedule({ fail(SocketTimeoutException("WebSocket close handshake timed out")) },
                CLOSE_TIMEOUT_MILLIS, TimeUnit.MILLISECONDS)
            synchronized(stateLock) { if (state == State.CLOSING) closeDeadline = task else task.cancel(false) }
        }

        private fun receiveClose(frame: CloseWebSocketFrame) {
            val closeNow = synchronized(stateLock) {
                if (state == State.TERMINATED) return
                peerClose = frame.statusCode() to frame.reasonText()
                if (state == State.CLOSING) true else {
                    state = State.CLOSING
                    outbound.addLast(PendingWrite(frame.retainedDuplicate(), null, closeAfter = true))
                    false
                }
            }
            if (closeNow) channel?.close()
            else { scheduleCloseDeadline(); scheduleWrites() }
        }

        private fun inactive() {
            val close = synchronized(stateLock) { peerClose }
            if (close == null) fail(IOException("WebSocket disconnected without a close handshake"))
            else terminate(null, close)
        }

        fun fail(error: Throwable, connectingOnly: Boolean = false) { terminate(error, null, connectingOnly) }

        private fun terminate(error: Throwable?, close: Pair<Int, String>?, connectingOnly: Boolean = false) {
            val resources = synchronized(stateLock) {
                if (state == State.TERMINATED || (connectingOnly && state != State.CONNECTING)) return
                state = State.TERMINATED
                deadline?.cancel(false)
                closeDeadline?.cancel(false)
                dns?.let { work ->
                    work.cancel(false)
                    if (work is Runnable) resolver.remove(work)
                }
                while (outbound.isNotEmpty()) {
                    val pending = outbound.removeFirst()
                    pending.bytes?.let(::unreserve)
                    ReferenceCountUtil.release(pending.frame)
                }
                channel to opened
            }
            resources.first?.close()
            synchronized(lock) { sessions.remove(this) }
            if (error != null) {
                ready.completeExceptionally(error)
                closedFuture.completeExceptionally(error)
            } else closedFuture.complete(null)
            if (resources.second) {
                try {
                    synchronized(callbackLock) {
                        if (error != null) listener.onError(this, error)
                        else listener.onClose(this, requireNotNull(close).first, close.second)
                    }
                } catch (_: Exception) {
                    // Terminal cleanup must survive an application's failing callback.
                }
            }
        }

        override fun isOpen(): Boolean = synchronized(stateLock) { state == State.OPEN }
        override fun subprotocol(): String = synchronized(stateLock) { selectedProtocol }
    }

    private class PendingWrite(val frame: WebSocketFrame, val bytes: Int?, val closeAfter: Boolean = false)

    private enum class State { CONNECTING, OPEN, CLOSING, TERMINATED }

    companion object {
        /** Maximum complete-message size used unless explicitly configured. */
        const val DEFAULT_MAXIMUM_MESSAGE_BYTES: Int = 1024 * 1024
        /** Maximum message bytes admitted but not yet written to the underlying connection. */
        const val DEFAULT_MAXIMUM_QUEUED_BYTES: Long = 4L * 1024 * 1024
        private const val MAXIMUM_MESSAGE_BYTES = 16 * 1024 * 1024
        private const val MAXIMUM_QUEUED_BYTES = 16L * 1024 * 1024
        private const val MAXIMUM_QUEUED_MESSAGES = 1024
        private const val MAXIMUM_HANDSHAKE_BYTES = 16 * 1024
        private const val CLOSE_TIMEOUT_MILLIS = 5_000L
        private val MANAGED_HEADERS = setOf(
            "Host", "Connection", "Upgrade", "Content-Length", "Transfer-Encoding", "Sec-WebSocket-Key",
            "Sec-WebSocket-Version", "Sec-WebSocket-Extensions",
        )

        /** Own a fresh NIO group and use the application's default JDK trust configuration. */
        @JvmStatic
        @JvmOverloads
        fun create(
            maximumMessageBytes: Int = DEFAULT_MAXIMUM_MESSAGE_BYTES,
            maximumQueuedBytes: Long = DEFAULT_MAXIMUM_QUEUED_BYTES,
        ): NettyWebSocketConnector {
            require(maximumMessageBytes in 1..MAXIMUM_MESSAGE_BYTES)
            require(maximumQueuedBytes in 1..MAXIMUM_QUEUED_BYTES)
            val tls = SSLContext.getDefault()
            return NettyWebSocketConnector(
                MultiThreadIoEventLoopGroup(1, daemonThreads("iroha-websocket-io"), NioIoHandler.newFactory()),
                tls, true, maximumMessageBytes, maximumQueuedBytes,
            )
        }

        private fun daemonThreads(name: String) = ThreadFactory { task -> Thread(task, name).apply { isDaemon = true } }
        private fun isProtocolToken(value: String): Boolean = value.isNotEmpty() && value.all {
            it.code in 33..126 && it !in "()<>@,;:\\\"/[]?={}"
        }
        private fun utf8Length(value: String): Int {
            var index = 0
            var bytes = 0
            while (index < value.length) {
                val ch = value[index++]
                bytes += when {
                    ch.code <= 0x7f -> 1
                    ch.code <= 0x7ff -> 2
                    Character.isHighSurrogate(ch) -> {
                        if (index == value.length || !Character.isLowSurrogate(value[index])) {
                            throw java.nio.charset.MalformedInputException(1)
                        }
                        index++
                        4
                    }
                    Character.isLowSurrogate(ch) -> throw java.nio.charset.MalformedInputException(1)
                    else -> 3
                }
            }
            return bytes
        }
        private fun utf8(value: String): ByteArray {
            val encoded = Charsets.UTF_8.newEncoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).encode(java.nio.CharBuffer.wrap(value))
            return ByteArray(encoded.remaining()).also { encoded.get(it) }
        }
        private fun sending(send: () -> Unit): CompletableFuture<Void> =
            try { send(); CompletableFuture.completedFuture(null) } catch (error: Exception) { failed(error) }
        private fun <T> failed(error: Throwable): CompletableFuture<T> =
            CompletableFuture<T>().also { it.completeExceptionally(error) }
    }
}
