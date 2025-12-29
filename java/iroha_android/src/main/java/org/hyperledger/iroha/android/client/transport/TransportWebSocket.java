package org.hyperledger.iroha.android.client.transport;

import java.nio.ByteBuffer;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/** Minimal transport-level WebSocket abstraction decoupled from {@code java.net.http.WebSocket}. */
public interface TransportWebSocket {

  CompletableFuture<Void> sendText(CharSequence data, boolean last);

  CompletableFuture<Void> sendBinary(ByteBuffer data, boolean last);

  /**
   * Sends a ping frame if supported by the underlying transport.
   *
   * <p>Implementations should return a failed future when the transport does not expose ping
   * controls (e.g., OkHttp), allowing callers to handle the unsupported path explicitly.
   */
  default CompletableFuture<Void> ping(final ByteBuffer data) {
    Objects.requireNonNull(data, "data");
    return unsupported("ping");
  }

  /**
   * Sends a pong frame if supported by the underlying transport.
   *
   * <p>Implementations should return a failed future when the transport does not expose pong
   * controls, mirroring {@link #ping(ByteBuffer)} semantics.
   */
  default CompletableFuture<Void> pong(final ByteBuffer data) {
    Objects.requireNonNull(data, "data");
    return unsupported("pong");
  }

  CompletableFuture<Void> close(int statusCode, String reason);

  /** Returns {@code true} when the underlying connection is open. */
  default boolean isOpen() {
    return false;
  }

  /** Returns the negotiated subprotocol (or empty string when none was negotiated). */
  default String subprotocol() {
    return "";
  }

  default void validateCloseCode(final int statusCode) {
    if (statusCode < 1000 || statusCode > 4999) {
      throw new IllegalArgumentException("Invalid WebSocket close status: " + statusCode);
    }
  }

  private CompletableFuture<Void> unsupported(final String operation) {
    final CompletableFuture<Void> failed = new CompletableFuture<>();
    failed.completeExceptionally(
        new UnsupportedOperationException(operation + " is not supported by this transport"));
    return failed;
  }

  /** Listener for WebSocket events. */
  interface Listener {
    default void onOpen(final TransportWebSocket socket) {}

    default void onText(final TransportWebSocket socket, final CharSequence data, final boolean last) {
      Objects.requireNonNull(data, "data");
    }

    default void onBinary(final TransportWebSocket socket, final ByteBuffer data, final boolean last) {
      Objects.requireNonNull(data, "data");
    }

    default void onPing(final TransportWebSocket socket, final ByteBuffer data) {
      Objects.requireNonNull(data, "data");
    }

    default void onPong(final TransportWebSocket socket, final ByteBuffer data) {
      Objects.requireNonNull(data, "data");
    }

    default void onError(final TransportWebSocket socket, final Throwable error) {
      Objects.requireNonNull(error, "error");
    }

    default void onClose(final TransportWebSocket socket, final int statusCode, final String reason) {}
  }
}
