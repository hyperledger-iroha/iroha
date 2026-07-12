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
) {
    /**
     * Returns the typed terminal error carried by this event, or `null` for ordinary events.
     *
     * A malformed `stream_error` frame throws [ToriiStreamProtocolException] instead of being
     * treated as an unrelated application event.
     */
    fun terminalStreamError(): ToriiStreamException? =
        if (event == "stream_error") ToriiStreamErrorParser.parse(data) else null
}
