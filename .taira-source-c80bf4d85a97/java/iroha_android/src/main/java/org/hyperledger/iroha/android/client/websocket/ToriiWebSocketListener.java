package org.hyperledger.iroha.android.client.websocket;

import java.nio.ByteBuffer;

/** Listener interface mirroring Torii WebSocket lifecycle events. */
public interface ToriiWebSocketListener {

  default void onOpen(final ToriiWebSocketSession session) {}

  default void onText(final ToriiWebSocketSession session, final CharSequence data, final boolean last) {}

  default void onBinary(final ToriiWebSocketSession session, final ByteBuffer data, final boolean last) {}

  default void onPing(final ToriiWebSocketSession session, final ByteBuffer message) {}

  default void onPong(final ToriiWebSocketSession session, final ByteBuffer message) {}

  default void onClose(final ToriiWebSocketSession session, final int statusCode, final String reason) {}

  default void onError(final ToriiWebSocketSession session, final Throwable error) {}
}
