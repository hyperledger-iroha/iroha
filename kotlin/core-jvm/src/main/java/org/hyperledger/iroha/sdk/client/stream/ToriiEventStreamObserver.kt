package org.hyperledger.iroha.sdk.client.stream

import java.time.Duration

/**
 * Observer hooks for `ToriiEventStreamSubscription`.
 *
 * Allows callers to capture telemetry about stream lifecycle events without mutating the primary
 * `ToriiEventStreamListener`.
 */
interface ToriiEventStreamObserver {

    /** Reason why the subscription scheduled a reconnect attempt. */
    enum class ReconnectReason {
        /** Initial connection attempt triggered by `ToriiEventStreamSubscription.start()`. */
        INITIAL,
        /** Stream closed normally (e.g., remote shutdown or idle timeout). */
        STREAM_CLOSED,
        /** Stream failed due to an IO/parse error. */
        STREAM_FAILURE,
    }

    /** Invoked when a new SSE stream has been established successfully. */
    fun onStreamOpened() {}

    /** Invoked when the active SSE stream terminates. */
    fun onStreamClosed() {}

    /** Invoked when the stream fails due to an unrecoverable error. */
    fun onStreamFailure(error: Throwable) {}

    /**
     * Invoked whenever the subscription schedules a reconnect attempt.
     *
     * @param delay Duration before the next attempt.
     * @param reason Context describing why the reconnect was requested.
     */
    fun onReconnectScheduled(delay: Duration, reason: ReconnectReason) {}
}
