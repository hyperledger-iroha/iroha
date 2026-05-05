package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayInputStream
import java.io.FilterInputStream
import java.io.IOException
import java.io.InputStream
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

    val body: InputStream = object : FilterInputStream(rawBody) {
        @Throws(IOException::class)
        override fun close() {
            this@TransportStreamResponse.close()
        }
    }

    val headers: Map<String, List<String>> get() = _headers

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        try {
            rawBody.close()
        } catch (_: IOException) {
        }
        onClose?.run()
    }

    companion object {
        private fun copyHeaders(source: Map<String, List<String>>?): Map<String, List<String>> {
            if (source == null) return emptyMap()
            val copy = TreeMap<String, List<String>>(String.CASE_INSENSITIVE_ORDER)
            for ((key, value) in source) {
                copy[key] = value?.toList() ?: emptyList()
            }
            return copy
        }
    }
}
