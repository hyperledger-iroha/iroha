package org.hyperledger.iroha.android.client.stream;

import java.time.Duration;

/** Listener invoked as server-sent event frames are received from Torii streaming endpoints. */
public interface ToriiEventStreamListener {

  /** Invoked after the stream has been established successfully. */
  default void onOpen() {}

  /** Invoked for every parsed {@link ServerSentEvent}. */
  void onEvent(ServerSentEvent event);

  /**
   * Invoked when the stream advertises a {@code retry} interval so callers can adjust reconnection
   * logic.
   */
  default void onRetryHint(final Duration duration) {}

  /** Invoked when the stream terminates normally. */
  default void onClosed() {}

  /** Invoked when the stream fails or parsing encounters an unrecoverable error. */
  default void onError(final Throwable error) {}
}
