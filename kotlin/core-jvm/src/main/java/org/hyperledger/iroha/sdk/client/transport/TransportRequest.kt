package org.hyperledger.iroha.sdk.client.transport

import java.net.URI
import java.time.Duration
import java.util.Locale

/** SDK-owned transport request wrapper to decouple callers from `java.net.http.HttpRequest`. */
class TransportRequest(
    @JvmField val method: String,
    @JvmField val uri: URI,
    headers: Map<String, List<String>>,
    body: ByteArray,
    /** Optional per-request timeout. A `null` value indicates executor defaults should apply. */
    @JvmField val timeout: Duration? = null,
    /** Optional inclusive buffered response-body limit. A `null` value uses the executor limit. */
    @JvmField val maximumResponseBytes: Long? = null,
    /** Whether the executor may consult ambient cookie or origin/proxy authentication sources. */
    @JvmField val allowAmbientCredentials: Boolean = true,
) {
    private val _headers: Map<String, List<String>> = copyHeaders(headers)
    private val _body: ByteArray = body.copyOf()

    /** Replay policy derived from the immutable request method, headers, and body. */
    @JvmField
    val replayPolicy: RequestReplayPolicy = deriveReplayPolicy(method, _headers, _body)

    init {
        require(maximumResponseBytes == null || maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
            "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
        }
    }

    constructor(
        method: String,
        uri: URI,
        headers: Map<String, List<String>>,
        body: ByteArray,
    ) : this(method, uri, headers, body, null, null)

    constructor(
        method: String,
        uri: URI,
        headers: Map<String, List<String>>,
        body: ByteArray,
        timeout: Duration?,
    ) : this(method, uri, headers, body, timeout, null)

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

        private fun deriveReplayPolicy(
            method: String,
            headers: Map<String, List<String>>,
            body: ByteArray,
        ): RequestReplayPolicy {
            val normalizedMethod = method.uppercase(Locale.ROOT)
            val readOnlyMethod = normalizedMethod == "GET" ||
                normalizedMethod == "HEAD" ||
                normalizedMethod == "OPTIONS"
            val carriesOneShotHeader = headers.keys.any { name ->
                when (name.lowercase(Locale.ROOT)) {
                    "x-iroha-signature",
                    "x-iroha-nonce",
                    "x-iroha-operator-nonce",
                    "x-iroha-operator-signature",
                    "x-iroha-onboarding-token" -> true
                    else -> false
                }
            }
            return if (readOnlyMethod && body.isEmpty() && !carriesOneShotHeader) {
                RequestReplayPolicy.RETRY_SAFE
            } else {
                RequestReplayPolicy.ONE_SHOT
            }
        }
    }

    class Builder {
        private var method: String = "GET"
        private var uri: URI = URI.create("http://localhost/")
        private val headers: MutableMap<String, MutableList<String>> = LinkedHashMap()
        private var body: ByteArray = ByteArray(0)
        private var timeout: Duration? = null
        private var maximumResponseBytes: Long? = null
        private var allowAmbientCredentials: Boolean = true

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
                    this.headers[key] = value.toMutableList()
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

        fun setMaximumResponseBytes(maximumResponseBytes: Long?): Builder {
            require(
                maximumResponseBytes == null ||
                    maximumResponseBytes in 1..Int.MAX_VALUE.toLong(),
            ) { "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}" }
            this.maximumResponseBytes = maximumResponseBytes
            return this
        }

        /** Controls whether the executor may inherit ambient credential sources. */
        fun setAllowAmbientCredentials(allowAmbientCredentials: Boolean): Builder {
            this.allowAmbientCredentials = allowAmbientCredentials
            return this
        }

        fun build(): TransportRequest {
            val headersCopy = LinkedHashMap<String, List<String>>()
            for ((key, value) in headers) {
                headersCopy[key] = value.toList()
            }
            return TransportRequest(
                method,
                uri,
                headersCopy,
                body,
                timeout,
                maximumResponseBytes,
                allowAmbientCredentials,
            )
        }
    }
}
