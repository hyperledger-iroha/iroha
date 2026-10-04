package org.hyperledger.iroha.sdk.client.stream

import java.time.Duration
import org.hyperledger.iroha.sdk.query.Filter

/**
 * Optional request customisations for Torii event streams.
 *
 * [filter] is the canonical text of an event filter (collection-query grammar over event fields
 * such as `tx_hash`, `tx_status` and `block_height`); it is sent as the `filter` parameter.
 * [timeout] bounds the idle time between bytes of an open stream (Torii sends a heartbeat every
 * 15 seconds); `null` uses the transport default of 45 seconds. A stream has no total lifetime.
 */
class ToriiEventStreamOptions @JvmOverloads constructor(
    queryParameters: Map<String, String> = emptyMap(),
    headers: Map<String, String> = emptyMap(),
    @JvmField val timeout: Duration? = null,
    @JvmField val filter: String? = null,
) {
    private val _queryParameters: Map<String, String> = queryParameters.toMap()
    private val _headers: Map<String, String> = headers.toMap()

    val queryParameters: Map<String, String> get() = _queryParameters
    val headers: Map<String, String> get() = _headers

    /** A builder pre-populated with these options. */
    fun toBuilder(): Builder = Builder()
        .queryParameters(_queryParameters)
        .headers(_headers)
        .setTimeout(timeout)
        .also { builder -> filter?.let(builder::setFilter) }

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
        private var filter: String? = null

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

        /** Maximum idle time between stream bytes; must be positive. */
        fun setTimeout(timeout: Duration?): Builder {
            require(timeout == null || (!timeout.isNegative && !timeout.isZero)) {
                "event stream idle timeout must be positive"
            }
            this.timeout = timeout
            return this
        }

        /**
         * Keep only events matching [filter]; it is sent in canonical text form, so object and
         * array literals (JSON-form only) are rejected.
         */
        fun setFilter(filter: Filter): Builder {
            filter.validate()
            org.hyperledger.iroha.sdk.query.FilterValidation.requireTextRepresentable(filter, "event stream filters")
            this.filter = filter.toString()
            return this
        }

        /** Keep only events matching the text filter [filter], sent unchanged. */
        fun setFilter(filter: String): Builder {
            require(filter.isNotBlank()) { "event filters must not be blank" }
            this.filter = filter
            return this
        }

        fun build(): ToriiEventStreamOptions =
            ToriiEventStreamOptions(queryParameters, headers, timeout, filter)
    }
}
