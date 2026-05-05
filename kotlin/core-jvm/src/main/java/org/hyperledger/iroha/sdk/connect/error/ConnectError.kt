package org.hyperledger.iroha.sdk.connect.error

import java.util.Optional

/** Canonical error payload surfaced by the Connect SDK. */
class ConnectError private constructor(builder: Builder) :
    RuntimeException(
        builder.message ?: requireNotNull(builder.code) { "code" },
        builder.cause,
    ),
    ConnectErrorConvertible {

    @JvmField val category: ConnectErrorCategory = requireNotNull(builder.category) { "category" }
    @JvmField val code: String = requireNotNull(builder.code) { "code" }
    @JvmField val fatal: Boolean = builder.fatal
    private val _httpStatus: Int? = builder.httpStatus
    private val _underlying: String? = builder.underlying

    fun httpStatus(): Optional<Int> = Optional.ofNullable(_httpStatus)

    fun underlying(): Optional<String> = Optional.ofNullable(_underlying)

    fun telemetryAttributes(): Map<String, String> =
        telemetryAttributes(ConnectErrorTelemetryOptions.empty())

    fun telemetryAttributes(options: ConnectErrorTelemetryOptions?): Map<String, String> {
        val effective = options ?: ConnectErrorTelemetryOptions.empty()
        val attributes = LinkedHashMap<String, String>()
        attributes["category"] = labelFor(category)
        attributes["code"] = code
        val fatalValue = effective.fatal ?: fatal
        attributes["fatal"] = fatalValue.toString()
        val httpValue = effective.httpStatus ?: _httpStatus
        if (httpValue != null) {
            attributes["http_status"] = httpValue.toString()
        }
        val underlyingValue = effective.underlying ?: _underlying
        if (!underlyingValue.isNullOrEmpty()) {
            attributes["underlying"] = underlyingValue
        }
        return attributes
    }

    override fun toConnectError(): ConnectError = this

    fun toBuilder(): Builder = Builder()
        .category(category)
        .code(code)
        .message(message)
        .fatal(fatal)
        .httpStatus(_httpStatus)
        .underlying(_underlying)
        .cause(cause)

    class Builder {
        internal var category: ConnectErrorCategory? = ConnectErrorCategory.INTERNAL
        internal var code: String? = "unknown_error"
        internal var message: String? = null
        internal var fatal: Boolean = false
        internal var httpStatus: Int? = null
        internal var underlying: String? = null
        internal var cause: Throwable? = null

        fun category(category: ConnectErrorCategory): Builder = apply { this.category = category }
        fun code(code: String?): Builder = apply { this.code = code ?: "unknown_error" }
        fun message(message: String?): Builder = apply { this.message = message }
        fun fatal(fatal: Boolean): Builder = apply { this.fatal = fatal }
        fun httpStatus(httpStatus: Int?): Builder = apply { this.httpStatus = httpStatus }
        fun underlying(underlying: String?): Builder = apply { this.underlying = underlying }
        fun cause(cause: Throwable?): Builder = apply { this.cause = cause }

        fun build(): ConnectError = ConnectError(this)
    }

    companion object {
        private const val serialVersionUID = 1L

        @JvmStatic
        fun builder(): Builder = Builder()

        private fun labelFor(category: ConnectErrorCategory): String = when (category) {
            ConnectErrorCategory.TRANSPORT -> "transport"
            ConnectErrorCategory.CODEC -> "codec"
            ConnectErrorCategory.AUTHORIZATION -> "authorization"
            ConnectErrorCategory.TIMEOUT -> "timeout"
            ConnectErrorCategory.QUEUE_OVERFLOW -> "queueOverflow"
            ConnectErrorCategory.INTERNAL -> "internal"
        }
    }
}
