package org.hyperledger.iroha.sdk.connect.error

/** Queue back-pressure / expiration events surfaced by Connect. */
class ConnectQueueError private constructor(
    val kind: Kind,
    @JvmField val limit: Int?,
    @JvmField val ttlMillis: Long?,
) : RuntimeException(
    if (kind == Kind.EXPIRED) "Connect queue entry expired" else "Connect queue overflow",
), ConnectErrorConvertible {

    enum class Kind {
        OVERFLOW,
        EXPIRED,
    }

    override fun toConnectError(): ConnectError {
        val builder = ConnectError.builder().message(message).cause(this)
        when (kind) {
            Kind.EXPIRED -> {
                builder.category(ConnectErrorCategory.TIMEOUT).code("queue.expired")
                if (ttlMillis != null) {
                    builder.underlying("ttlMs=$ttlMillis")
                }
            }
            Kind.OVERFLOW -> {
                builder.category(ConnectErrorCategory.QUEUE_OVERFLOW).code("queue.overflow")
                if (limit != null) {
                    builder.underlying("limit=$limit")
                }
            }
        }
        return builder.build()
    }

    companion object {
        private const val serialVersionUID = 1L

        @JvmStatic
        fun overflow(limit: Int?): ConnectQueueError =
            ConnectQueueError(Kind.OVERFLOW, limit, null)

        @JvmStatic
        fun expired(ttlMillis: Long?): ConnectQueueError =
            ConnectQueueError(Kind.EXPIRED, null, ttlMillis)
    }
}
