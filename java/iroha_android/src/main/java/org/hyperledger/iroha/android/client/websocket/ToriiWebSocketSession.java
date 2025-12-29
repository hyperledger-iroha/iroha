package org.hyperledger.iroha.android.client.websocket;

import java.nio.ByteBuffer;
import java.util.concurrent.CompletableFuture;

/** Handle for an active Torii WebSocket session. */
public interface ToriiWebSocketSession extends AutoCloseable {

  int NORMAL_CLOSURE = 1000;

  /** Sends a text frame. */
  CompletableFuture<Void> sendText(CharSequence data, boolean last);

  /** Sends a binary frame. */
  CompletableFuture<Void> sendBinary(ByteBuffer data, boolean last);

  /** Sends a ping frame. */
  CompletableFuture<Void> sendPing(ByteBuffer message);

  /** Sends a pong frame. */
  CompletableFuture<Void> sendPong(ByteBuffer message);

  /** Initiates a close handshake. */
  CompletableFuture<Void> close(int statusCode, String reason);

  /** Returns {@code true} when the underlying transport is still open. */
  boolean isOpen();

  /** Returns the negotiated subprotocol (if any). */
  String subprotocol();

  @Override
  default void close() {
    close(NORMAL_CLOSURE, "client closing");
  }
}
