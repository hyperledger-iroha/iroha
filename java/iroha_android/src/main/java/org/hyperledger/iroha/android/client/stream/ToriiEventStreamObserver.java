package org.hyperledger.iroha.android.client.stream;

import java.time.Duration;

/**
 * Observer hooks for {@link ToriiEventStreamSubscription}.
 *
 * <p>Allows callers to capture telemetry about stream lifecycle events without mutating the primary
 * {@link ToriiEventStreamListener}.
 */
public interface ToriiEventStreamObserver {

  /** Reason why the subscription scheduled a reconnect attempt. */
  enum ReconnectReason {
    /** Initial connection attempt triggered by {@link ToriiEventStreamSubscription#start()}. */
    INITIAL,
    /** Stream closed normally (e.g., remote shutdown or idle timeout). */
    STREAM_CLOSED,
    /** Stream failed due to an IO/parse error. */
    STREAM_FAILURE
  }

  /** Invoked when a new SSE stream has been established successfully. */
  default void onStreamOpened() {}

  /** Invoked when the active SSE stream terminates. */
  default void onStreamClosed() {}

  /** Invoked when the stream fails due to an unrecoverable error. */
  default void onStreamFailure(final Throwable error) {}

  /**
   * Invoked whenever the subscription schedules a reconnect attempt.
   *
   * @param delay Duration before the next attempt.
   * @param reason Context describing why the reconnect was requested.
   */
  default void onReconnectScheduled(final Duration delay, final ReconnectReason reason) {}
}
