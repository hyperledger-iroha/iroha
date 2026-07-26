package org.hyperledger.iroha.sdk.client.websocket

import java.time.Duration

/**
 * Observer hooks for `ToriiWebSocketSubscription` lifecycle events.
 *
 * Provides telemetry-friendly callbacks without mutating the primary `ToriiWebSocketListener`.
 */
interface ToriiWebSocketObserver {

    /** Reason a reconnect attempt was scheduled. */
    enum class ReconnectReason {
        /** Initial connection attempt triggered by `ToriiWebSocketSubscription.start()`. */
        INITIAL,
        /** Session closed normally (remote close or idle timeout). */
        SESSION_CLOSED,
        /** Session failed due to an IO/protocol error. */
        SESSION_FAILURE,
    }

    /** Invoked when a WebSocket session has been established successfully. */
    fun onSessionOpened() {}

    /** Invoked when the active WebSocket session terminates. */
    fun onSessionClosed() {}

    /** Invoked when the session fails due to an unrecoverable error. */
    fun onSessionFailure(error: Throwable) {}

    /**
     * Invoked whenever the subscription schedules a reconnect attempt.
     *
     * @param delay Duration before the next attempt.
     * @param reason Context describing why the reconnect was requested.
     */
    fun onReconnectScheduled(delay: Duration, reason: ReconnectReason) {}
}
