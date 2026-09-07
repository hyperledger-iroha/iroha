package org.hyperledger.iroha.sdk.client.transport

import java.io.IOException
import java.time.Duration
import java.util.concurrent.AbstractExecutorService
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executor
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import okhttp3.Authenticator
import okhttp3.Call
import okhttp3.Callback
import okhttp3.Dispatcher
import okhttp3.Headers
import okhttp3.MediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.Response
import okio.BufferedSink
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor

/**
 * Canonical JVM/Android HTTP and SSE transport. Each instance owns its calls and open streams.
 * Cancelling a future cancels that call; [close] permanently rejects new calls and disposes every
 * active call/stream. Constructing with an application [OkHttpClient] borrows its pool, dispatcher,
 * cache and TLS configuration. [create] owns those resources; an injected scheduling [Executor]
 * always stays application-owned. There is no global client or runtime transport discovery.
 *
 * Redirects and authentication follow-ups are disabled. ONE_SHOT requests never retry a dispatch,
 * including a signed GET receiving Retry-After: 0. Buffered limits apply after gzip decompression.
 */
class OkHttpTransportExecutor private constructor(
    sourceClient: OkHttpClient,
    private val ownsClient: Boolean,
    private val maximumResponseBytes: Long,
) : HttpTransportExecutor, StreamingTransportExecutor {
    private val lock = Any()
    private var closed = false
    private val active = LinkedHashMap<Call, Pending<*>>()

    private val client: OkHttpClient
    private val oneShotClient: OkHttpClient

    init {
        require(maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
            "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
        }
        require(sourceClient.interceptors.isEmpty() && sourceClient.networkInterceptors.isEmpty()) {
            "Inject a client without interceptors; request dispatch and signing policy belong to this transport"
        }
        client = sourceClient.newBuilder()
            .followRedirects(false).followSslRedirects(false)
            .authenticator(Authenticator.NONE).proxyAuthenticator(Authenticator.NONE)
            .addInterceptor { chain ->
                val response = chain.proceed(chain.request())
                val retryAfter = chain.request().tag(DispatchGuard::class.java)?.retryAfter
                if (retryAfter == null) response else response.newBuilder()
                    .header("Retry-After", retryAfter).build()
            }
            .addNetworkInterceptor { chain ->
                val guard = chain.request().tag(DispatchGuard::class.java)
                if (guard != null && !guard.dispatched.compareAndSet(false, true)) {
                    throw IOException("ONE_SHOT HTTP request cannot be redispatched")
                }
                val response = chain.proceed(chain.request())
                try {
                    val headers = response.headers.toMultimap()
                    BoundedResponseBodyReader.rejectAmbiguousFraming(
                        headers, BoundedResponseBodyReader.canonicalContentLength(headers),
                    )
                    // OkHttp retries 503 + Retry-After: 0 even with connection retries disabled.
                    // Suppress the internal hint, then restore it at the application boundary.
                    if (guard != null && response.code == 503 && response.header("Retry-After") == "0") {
                        guard.retryAfter = "0"
                        response.newBuilder().removeHeader("Retry-After").build()
                    } else response
                } catch (failure: Throwable) {
                    response.close()
                    throw failure
                }
            }.build()
        oneShotClient = client.newBuilder().retryOnConnectionFailure(false).cache(null).build()
    }

    /** Borrow an application client. Closing this adapter never shuts down that client's resources. */
    @JvmOverloads
    constructor(client: OkHttpClient, maximumResponseBytes: Long = DEFAULT_MAXIMUM_RESPONSE_BYTES) :
        this(client, false, maximumResponseBytes)

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        dispatch(request, streaming = false) { response, _ ->
            response.use {
                TransportResponse(
                    response.code,
                    BoundedResponseBodyReader.read(
                        response.body?.byteStream(), response.headers.toMultimap(),
                        request.method, response.code,
                        minOf(maximumResponseBytes, request.maximumResponseBytes ?: maximumResponseBytes),
                    ),
                    response.message, response.headers.toMultimap(), response.request.url.toUri(),
                    redirected = false,
                )
            }
        }

    override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
        dispatch(request, streaming = true) { response, call ->
            TransportStreamResponse(
                response.code, response.body?.byteStream(), response.message,
                response.headers.toMultimap(), Runnable {
                    call.cancel()
                    synchronized(lock) { active.remove(call) }
                },
            )
        }

    private fun <T> dispatch(
        request: TransportRequest,
        streaming: Boolean,
        decode: (Response, Call) -> T,
    ): CompletableFuture<T> {
        val future = CompletableFuture<T>()
        val call: Call
        try {
            val selected = if (request.replayPolicy == RequestReplayPolicy.ONE_SHOT) oneShotClient else client
            call = selected.newCall(buildRequest(request))
            request.timeout?.let { timeout ->
                require(!timeout.isNegative) { "timeout must be non-negative" }
                call.timeout().timeout(timeout.toNanos(), TimeUnit.NANOSECONDS)
            }
        } catch (failure: Exception) {
            future.completeExceptionally(failure)
            return future
        }
        val pending = Pending(future)
        synchronized(lock) {
            if (closed) {
                future.completeExceptionally(IllegalStateException("HTTP transport is closed"))
                return future
            }
            active[call] = pending
        }
        future.whenComplete { _, _ ->
            if (future.isCancelled) {
                call.cancel()
                synchronized(lock) { active.remove(call) }?.stream?.close()
            }
        }
        try {
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, e: IOException) {
                    synchronized(lock) { active.remove(call) }
                    future.completeExceptionally(e)
                }

                override fun onResponse(call: Call, response: Response) {
                    var transferred = false
                    try {
                        if (future.isDone) return
                        val value = decode(response, call)
                        if (streaming) {
                            val stream = value as TransportStreamResponse
                            val retained = synchronized(lock) {
                                if (closed || active[call] !== pending || future.isDone) false else {
                                    pending.stream = stream
                                    true
                                }
                            }
                            if (retained && future.complete(value)) transferred = true else stream.close()
                        } else future.complete(value)
                    } catch (failure: Exception) {
                        future.completeExceptionally(failure)
                    } finally {
                        if (!transferred) {
                            response.close()
                            synchronized(lock) { active.remove(call) }
                        }
                    }
                }
            })
        } catch (failure: Exception) {
            call.cancel()
            synchronized(lock) { active.remove(call) }
            future.completeExceptionally(failure)
        }
        return future
    }

    override fun close() {
        val pending = synchronized(lock) {
            if (closed) return
            closed = true
            active.toMap().also { active.clear() }
        }
        for ((call, operation) in pending) {
            call.cancel()
            operation.future.cancel(false)
            operation.stream?.close()
        }
        if (ownsClient) {
            client.connectionPool.evictAll()
            client.cache?.close()
            client.dispatcher.executorService.shutdown()
        }
    }

    private fun buildRequest(request: TransportRequest): Request {
        val headers = Headers.Builder()
        for ((name, values) in request.headers) for (value in values) {
            headers.addUnsafeNonAscii(name, value)
        }
        val bytes = request.body
        val body = if (request.method.equals("GET", true) || request.method.equals("HEAD", true)) {
            require(bytes.isEmpty()) { "GET and HEAD requests cannot contain a body" }
            null
        } else object : RequestBody() {
            override fun contentType(): MediaType? = null
            override fun contentLength(): Long = bytes.size.toLong()
            override fun isOneShot(): Boolean = request.replayPolicy == RequestReplayPolicy.ONE_SHOT
            override fun writeTo(sink: BufferedSink) { sink.write(bytes) }
        }
        return Request.Builder().url(request.uri.toURL()).method(request.method, body)
            .headers(headers.build())
            .apply {
                if (request.replayPolicy == RequestReplayPolicy.ONE_SHOT) {
                    tag(DispatchGuard::class.java, DispatchGuard())
                }
            }.build()
    }

    private class Pending<T>(val future: CompletableFuture<T>) {
        @Volatile var stream: TransportStreamResponse? = null
    }

    private class DispatchGuard {
        val dispatched = AtomicBoolean(false)
        @Volatile var retryAfter: String? = null
    }

    companion object {
        /** Maximum decoded buffered body, in bytes (64 MiB). */
        const val DEFAULT_MAXIMUM_RESPONSE_BYTES: Long = 64L * 1024L * 1024L

        /** Create an owned HTTP adapter. A supplied scheduling executor remains borrowed. */
        @JvmStatic
        @JvmOverloads
        fun create(
            connectTimeout: Duration? = null,
            readTimeout: Duration? = null,
            asyncExecutor: Executor? = null,
            maximumResponseBytes: Long = DEFAULT_MAXIMUM_RESPONSE_BYTES,
        ): OkHttpTransportExecutor {
            val builder = OkHttpClient.Builder()
            connectTimeout?.let { builder.connectTimeout(it) }
            readTimeout?.let { builder.readTimeout(it) }
            asyncExecutor?.let { builder.dispatcher(Dispatcher(BorrowedExecutor(it))) }
            return OkHttpTransportExecutor(builder.build(), true, maximumResponseBytes)
        }
    }

    /** Dispatcher lifecycle adapter; it never shuts down the application executor. */
    private class BorrowedExecutor(private val delegate: Executor) : AbstractExecutorService() {
        private val monitor = Object()
        private var stopped = false
        private var running = 0
        override fun execute(command: Runnable) {
            synchronized(monitor) {
                if (stopped) throw RejectedExecutionException("HTTP dispatcher is closed")
                running++
            }
            val counted = AtomicBoolean(true)
            try {
                delegate.execute {
                    try { command.run() } finally { if (counted.getAndSet(false)) finished() }
                }
            } catch (failure: RuntimeException) {
                if (counted.getAndSet(false)) finished()
                throw failure
            }
        }
        private fun finished() = synchronized(monitor) { running--; monitor.notifyAll() }
        override fun shutdown() = synchronized(monitor) { stopped = true; monitor.notifyAll() }
        override fun shutdownNow(): MutableList<Runnable> { shutdown(); return ArrayList() }
        override fun isShutdown(): Boolean = synchronized(monitor) { stopped }
        override fun isTerminated(): Boolean = synchronized(monitor) { stopped && running == 0 }
        override fun awaitTermination(timeout: Long, unit: TimeUnit): Boolean {
            val limit = unit.toNanos(timeout)
            val start = System.nanoTime()
            synchronized(monitor) {
                while (!isTerminated) {
                    val remaining = limit - (System.nanoTime() - start)
                    if (remaining <= 0) return false
                    TimeUnit.NANOSECONDS.timedWait(monitor, remaining)
                }
                return true
            }
        }
    }
}
