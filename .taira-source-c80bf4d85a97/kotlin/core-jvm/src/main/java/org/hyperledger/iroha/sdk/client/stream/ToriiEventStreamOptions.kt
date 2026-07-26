package org.hyperledger.iroha.sdk.client.stream

import java.time.Duration

/** Optional request customisations for Torii event streams. */
class ToriiEventStreamOptions(
    queryParameters: Map<String, String> = emptyMap(),
    headers: Map<String, String> = emptyMap(),
    @JvmField val timeout: Duration? = null,
) {
    private val _queryParameters: Map<String, String> = queryParameters.toMap()
    private val _headers: Map<String, String> = headers.toMap()

    val queryParameters: Map<String, String> get() = _queryParameters
    val headers: Map<String, String> get() = _headers

    companion object {
        @JvmStatic
        fun defaultOptions(): ToriiEventStreamOptions = ToriiEventStreamOptions()

        @JvmStatic
        fun builder(): Builder = Builder()
    }

    class Builder {
        private val queryParameters: MutableMap<String, String> = LinkedHashMap()
        private val headers: MutableMap<String, String> = LinkedHashMap()
        private var timeout: Duration? = null

        fun putQueryParameter(key: String, value: String): Builder {
            queryParameters[key] = value
            return this
        }

        fun queryParameters(parameters: Map<String, String>?): Builder {
            queryParameters.clear()
            parameters?.forEach { (k, v) -> putQueryParameter(k, v) }
            return this
        }

        fun putHeader(name: String, value: String): Builder {
            headers[name] = value
            return this
        }

        fun headers(values: Map<String, String>?): Builder {
            headers.clear()
            values?.forEach { (k, v) -> putHeader(k, v) }
            return this
        }

        fun setTimeout(timeout: Duration?): Builder {
            this.timeout = if (timeout == null) null
            else if (timeout.isNegative) Duration.ZERO
            else timeout
            return this
        }

        fun build(): ToriiEventStreamOptions =
            ToriiEventStreamOptions(queryParameters, headers, timeout)
    }
}
