package org.hyperledger.iroha.sdk.client.transport

import java.net.URI
import java.time.Duration

/** SDK-owned transport request wrapper to decouple callers from `java.net.http.HttpRequest`. */
class TransportRequest(
    @JvmField val method: String,
    @JvmField val uri: URI,
    headers: Map<String, List<String>>,
    body: ByteArray,
    /** Optional per-request timeout. A `null` value indicates executor defaults should apply. */
    @JvmField val timeout: Duration? = null,
) {
    private val _headers: Map<String, List<String>> = copyHeaders(headers)
    private val _body: ByteArray = body.copyOf()

    val headers: Map<String, List<String>> get() = _headers
    val body: ByteArray get() = _body.copyOf()

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun copyHeaders(source: Map<String, List<String>>): Map<String, List<String>> {
            val copy = LinkedHashMap<String, List<String>>()
            for ((key, value) in source) {
                copy[key] = value.toList()
            }
            return copy
        }
    }

    class Builder {
        private var method: String = "GET"
        private var uri: URI = URI.create("http://localhost/")
        private val headers: MutableMap<String, MutableList<String>> = LinkedHashMap()
        private var body: ByteArray = ByteArray(0)
        private var timeout: Duration? = null

        fun setMethod(method: String): Builder {
            this.method = method
            return this
        }

        fun setUri(uri: URI): Builder {
            this.uri = uri
            return this
        }

        fun addHeader(name: String, value: String): Builder {
            headers.getOrPut(name) { ArrayList() }.add(value)
            return this
        }

        fun setHeaders(headers: Map<String, List<String>>?): Builder {
            this.headers.clear()
            if (headers != null) {
                for ((key, value) in headers) {
                    this.headers[key] = value?.toMutableList() ?: mutableListOf()
                }
            }
            return this
        }

        fun setBody(body: ByteArray?): Builder {
            this.body = body?.copyOf() ?: ByteArray(0)
            return this
        }

        fun setTimeout(timeout: Duration?): Builder {
            if (timeout != null) {
                require(!timeout.isNegative) { "timeout must be non-negative" }
            }
            this.timeout = timeout
            return this
        }

        fun build(): TransportRequest {
            val headersCopy = LinkedHashMap<String, List<String>>()
            for ((key, value) in headers) {
                headersCopy[key] = value.toList()
            }
            return TransportRequest(method, uri, headersCopy, body, timeout)
        }
    }
}
