package org.hyperledger.iroha.sdk.client.transport

import java.net.URI
import java.util.Collections
import java.util.TreeMap

/**
 * SDK-owned transport response wrapper to decouple callers from `java.net.http.HttpResponse`.
 *
 * [finalUri] and [redirected] carry network provenance for response paths that must remain bound
 * to an exact signed URI. Callers must provide both fields explicitly; [finalUri] is `null` only
 * when the response source genuinely cannot establish network provenance.
 */
class TransportResponse(
    @JvmField val statusCode: Int,
    body: ByteArray?,
    message: String?,
    headers: Map<String, List<String>>?,
    @JvmField val finalUri: URI?,
    @JvmField val redirected: Boolean,
) {
    @JvmField val message: String = message ?: ""

    private val _body: ByteArray = body?.copyOf() ?: ByteArray(0)
    private val _headers: Map<String, List<String>> = copyHeaders(headers)

    val body: ByteArray get() = _body.copyOf()
    val headers: Map<String, List<String>> get() = _headers

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun copyHeaders(source: Map<String, List<String>>?): Map<String, List<String>> {
            if (source == null) return emptyMap()
            val merged = TreeMap<String, MutableList<String>>(String.CASE_INSENSITIVE_ORDER)
            for ((key, value) in source) {
                merged.getOrPut(key) { ArrayList() }.addAll(value.orEmpty())
            }
            val copy = TreeMap<String, List<String>>(String.CASE_INSENSITIVE_ORDER)
            for ((name, values) in merged) {
                copy[name] = Collections.unmodifiableList(ArrayList(values))
            }
            return Collections.unmodifiableMap(copy)
        }
    }

    class Builder {
        private var statusCode: Int = 0
        private var body: ByteArray = ByteArray(0)
        private var message: String = ""
        private val headers: MutableMap<String, MutableList<String>> = LinkedHashMap()
        private var finalUri: URI? = null
        private var redirected: Boolean = false

        fun setStatusCode(statusCode: Int): Builder {
            this.statusCode = statusCode
            return this
        }

        fun setBody(body: ByteArray?): Builder {
            this.body = body?.copyOf() ?: ByteArray(0)
            return this
        }

        fun setMessage(message: String?): Builder {
            this.message = message ?: ""
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
                    this.headers[key] = value.orEmpty().toMutableList()
                }
            }
            return this
        }

        /** Records the final network URI and whether any redirect was followed. */
        fun setNetworkProvenance(finalUri: URI, redirected: Boolean): Builder {
            this.finalUri = finalUri
            this.redirected = redirected
            return this
        }

        fun build(): TransportResponse =
            TransportResponse(statusCode, body, message, headers, finalUri, redirected)
    }
}
