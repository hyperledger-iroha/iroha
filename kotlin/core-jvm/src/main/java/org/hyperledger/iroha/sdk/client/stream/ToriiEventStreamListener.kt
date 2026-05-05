package org.hyperledger.iroha.sdk.client.stream

import java.time.Duration

/** Listener invoked as server-sent event frames are received from Torii streaming endpoints. */
interface ToriiEventStreamListener {

    /** Invoked after the stream has been established successfully. */
    fun onOpen() {}

    /** Invoked for every parsed `ServerSentEvent`. */
    fun onEvent(event: ServerSentEvent)

    /**
     * Invoked when the stream advertises a `retry` interval so callers can adjust reconnection
     * logic.
     */
    fun onRetryHint(duration: Duration) {}

    /** Invoked when the stream terminates normally. */
    fun onClosed() {}

    /** Invoked when the stream fails or parsing encounters an unrecoverable error. */
    fun onError(error: Throwable) {}
}
