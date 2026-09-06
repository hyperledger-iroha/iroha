package org.hyperledger.iroha.sdk.client.transport

import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor

/** Per-client call lifetime; injected backends are borrowed, default backends are owned. */
internal open class HttpTransportScope private constructor(
    protected val backend: TransportExecutor,
    private val ownsBackend: Boolean,
) : HttpTransportExecutor {
    private val lock = Any()
    private var closed = false
    private val pending = LinkedHashSet<Operation>()

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        dispatch(streaming = false) { backend.execute(request) }

    protected fun <T> dispatch(streaming: Boolean, start: () -> CompletableFuture<T>): CompletableFuture<T> {
        val result = CompletableFuture<T>()
        val operation = Operation(result)
        synchronized(lock) {
            if (closed) {
                result.completeExceptionally(IllegalStateException("HTTP client is closed"))
                return result
            }
            pending.add(operation)
        }
        result.whenComplete { _, _ -> if (result.isCancelled) cancel(operation) }
        val upstream = try { start() } catch (failure: Exception) {
            synchronized(lock) { pending.remove(operation) }
            result.completeExceptionally(failure)
            return result
        }
        val admitted = synchronized(lock) {
            operation.upstream = upstream
            operation in pending
        }
        if (!admitted) upstream.cancel(false)
        upstream.whenComplete { value, error ->
            if (error != null) {
                synchronized(lock) { pending.remove(operation) }
                if (upstream.isCancelled) result.cancel(false) else result.completeExceptionally(error)
            } else if (streaming) {
                val source = value as TransportStreamResponse
                val scoped = TransportStreamResponse(
                    source.statusCode, source.body, source.message, source.headers,
                    Runnable {
                        try { source.close() } finally { synchronized(lock) { pending.remove(operation) } }
                    },
                )
                val retained = synchronized(lock) {
                    if (operation !in pending || closed) false else {
                        operation.stream = scoped
                        true
                    }
                }
                @Suppress("UNCHECKED_CAST")
                if (!retained || !result.complete(scoped as T)) scoped.close()
            } else {
                synchronized(lock) { pending.remove(operation) }
                result.complete(value)
            }
        }
        return result
    }

    private fun cancel(operation: Operation) {
        val resources = synchronized(lock) {
            pending.remove(operation)
            operation.upstream to operation.stream
        }
        resources.first?.cancel(false)
        resources.second?.close()
    }

    override fun close() {
        val operations = synchronized(lock) {
            if (closed) return
            closed = true
            pending.toList().also { pending.clear() }
        }
        var failure: Exception? = null
        fun dispose(action: () -> Unit) {
            try { action() } catch (error: Exception) {
                if (failure == null) failure = error else if (failure !== error) failure!!.addSuppressed(error)
            }
        }
        for (operation in operations) {
            dispose { operation.result.cancel(false) }
            dispose { cancel(operation) }
        }
        if (ownsBackend) dispose { (backend as AutoCloseable).close() }
        failure?.let { throw it }
    }

    private class Operation(val result: CompletableFuture<*>) {
        var upstream: CompletableFuture<*>? = null
        var stream: TransportStreamResponse? = null
    }

    private class StreamingScope(backend: StreamingTransportExecutor, owns: Boolean) :
        HttpTransportScope(backend, owns), StreamingTransportExecutor {
        override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
            dispatch(streaming = true) { (backend as StreamingTransportExecutor).openStream(request) }
    }

    companion object {
        /** Borrow injected backends; create and own a backend when none is supplied. */
        fun create(backend: TransportExecutor? = null): HttpTransportScope =
            bind(backend ?: OkHttpTransportExecutor.create(), owns = backend == null)

        /** Transfer a newly constructed backend to its one owning client. */
        fun own(backend: HttpTransportExecutor): HttpTransportScope = bind(backend, owns = true)

        private fun bind(backend: TransportExecutor, owns: Boolean): HttpTransportScope =
            if (backend is StreamingTransportExecutor) StreamingScope(backend, owns)
            else HttpTransportScope(backend, owns)
    }
}
