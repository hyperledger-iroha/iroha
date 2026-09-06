package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayInputStream
import java.io.FilterInputStream
import java.io.IOException
import java.io.InputStream
import java.util.Collections
import java.util.TreeMap
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Streaming transport response that exposes the response body without pre-buffering.
 */
class TransportStreamResponse(
    @JvmField val statusCode: Int,
    body: InputStream?,
    message: String?,
    headers: Map<String, List<String>>?,
    private val onClose: Runnable?,
) : AutoCloseable {

    @JvmField val message: String = message ?: ""

    private val rawBody: InputStream = body ?: ByteArrayInputStream(ByteArray(0))
    private val _headers: Map<String, List<String>> = copyHeaders(headers)
    private val closed = AtomicBoolean(false)
    private val ioLock = Any()

    val body: InputStream = object : FilterInputStream(rawBody) {
        override fun read(): Int = synchronized(ioLock) { requireOpen(); rawBody.read() }
        override fun read(bytes: ByteArray, offset: Int, length: Int): Int = synchronized(ioLock) {
            requireOpen()
            rawBody.read(bytes, offset, length)
        }
        override fun skip(count: Long): Long = synchronized(ioLock) { requireOpen(); rawBody.skip(count) }
        override fun available(): Int = synchronized(ioLock) { requireOpen(); rawBody.available() }
        @Throws(IOException::class)
        override fun close() {
            this@TransportStreamResponse.close()
        }
    }

    val headers: Map<String, List<String>> get() = _headers

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        try {
            // Cancel the underlying call before closing a body being read on another thread.
            onClose?.run()
        } finally {
            // The transport cancellation above wakes a blocked read before disposal acquires
            // this lock. Okio response sources cannot be read and closed concurrently.
            synchronized(ioLock) {
                try {
                    rawBody.close()
                } catch (_: IOException) {
                }
            }
        }
    }

    private fun requireOpen() {
        if (closed.get()) throw IOException("HTTP response stream is closed")
    }

    companion object {
        private fun copyHeaders(source: Map<String, List<String>>?): Map<String, List<String>> {
            if (source == null) return emptyMap()
            val copy = TreeMap<String, List<String>>(String.CASE_INSENSITIVE_ORDER)
            for ((key, value) in source) {
                val incoming = value.toList()
                val existing = copy[key]
                copy[key] = Collections.unmodifiableList(if (existing == null) incoming else existing + incoming)
            }
            return Collections.unmodifiableMap(copy)
        }
    }
}
