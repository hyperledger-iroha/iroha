package org.hyperledger.iroha.android.client.websocket;

import java.time.Duration;

/**
 * Observer hooks for {@link ToriiWebSocketSubscription} lifecycle events.
 *
 * <p>Provides telemetry-friendly callbacks without mutating the primary {@link ToriiWebSocketListener}.
 */
public interface ToriiWebSocketObserver {

  /** Reason a reconnect attempt was scheduled. */
  enum ReconnectReason {
    /** Initial connection attempt triggered by {@link ToriiWebSocketSubscription#start()}. */
    INITIAL,
    /** Session closed normally (remote close or idle timeout). */
    SESSION_CLOSED,
    /** Session failed due to an IO/protocol error. */
    SESSION_FAILURE
  }

  /** Invoked when a WebSocket session has been established successfully. */
  default void onSessionOpened() {}

  /** Invoked when the active WebSocket session terminates. */
  default void onSessionClosed() {}

  /** Invoked when the session fails due to an unrecoverable error. */
  default void onSessionFailure(final Throwable error) {}

  /**
   * Invoked whenever the subscription schedules a reconnect attempt.
   *
   * @param delay Duration before the next attempt.
   * @param reason Context describing why the reconnect was requested.
   */
  default void onReconnectScheduled(final Duration delay, final ReconnectReason reason) {}
}
