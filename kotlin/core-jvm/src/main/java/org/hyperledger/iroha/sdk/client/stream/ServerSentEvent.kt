package org.hyperledger.iroha.sdk.client.stream

/** Representation of a single server-sent event frame. */
class ServerSentEvent internal constructor(
    /** Name of the event (defaults to `message` when unspecified). */
    @JvmField val event: String,
    /**
     * Raw data payload. When a frame contains multiple `data:` lines, they are joined with
     * `'\n'` as mandated by the SSE specification.
     */
    @JvmField val data: String,
    /** Event identifier supplied via the `id:` field (may be `null`). */
    @JvmField val id: String?,
)
