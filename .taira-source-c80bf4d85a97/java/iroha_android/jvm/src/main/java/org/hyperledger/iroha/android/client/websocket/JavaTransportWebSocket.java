package org.hyperledger.iroha.android.client.websocket;

import java.net.http.WebSocket;
import java.nio.ByteBuffer;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionStage;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;

/**
 * Adapter between {@link WebSocket} and the SDK {@link TransportWebSocket} interface.
 *
 * <p>JDK {@link WebSocket} instances deliver events on the selector thread; listeners should avoid
 * blocking to prevent backpressure on the HTTP client.
 */
final class JavaTransportWebSocket implements TransportWebSocket, WebSocket.Listener {
  private final TransportWebSocket.Listener listener;
  private final CompletableFuture<WebSocket> ready;
  private volatile WebSocket delegate;

  JavaTransportWebSocket(final TransportWebSocket.Listener listener) {
    this.listener = Objects.requireNonNull(listener, "listener");
    this.ready = new CompletableFuture<>();
  }

  void attach(final WebSocket webSocket) {
    delegate = Objects.requireNonNull(webSocket, "webSocket");
    ready.complete(webSocket);
    webSocket.request(1);
    listener.onOpen(this);
  }

  @Override
  public void onOpen(final WebSocket webSocket) {
    webSocket.request(1);
  }

  @Override
  public CompletionStage<?> onText(final WebSocket webSocket, final CharSequence data, final boolean last) {
    listener.onText(this, data, last);
    return WebSocket.Listener.super.onText(webSocket, data, last);
  }

  @Override
  public CompletionStage<?> onBinary(final WebSocket webSocket, final ByteBuffer data, final boolean last) {
    listener.onBinary(this, data, last);
    return WebSocket.Listener.super.onBinary(webSocket, data, last);
  }

  @Override
  public CompletionStage<?> onPing(final WebSocket webSocket, final ByteBuffer message) {
    listener.onPing(this, message);
    return WebSocket.Listener.super.onPing(webSocket, message);
  }

  @Override
  public CompletionStage<?> onPong(final WebSocket webSocket, final ByteBuffer message) {
    listener.onPong(this, message);
    return WebSocket.Listener.super.onPong(webSocket, message);
  }

  @Override
  public CompletionStage<?> onClose(final WebSocket webSocket, final int statusCode, final String reason) {
    listener.onClose(this, statusCode, reason);
    return WebSocket.Listener.super.onClose(webSocket, statusCode, reason);
  }

  @Override
  public void onError(final WebSocket webSocket, final Throwable error) {
    listener.onError(this, error);
  }

  @Override
  public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
    Objects.requireNonNull(data, "data");
    return ready.thenCompose(ws -> ws.sendText(data.toString(), last).thenApply(ignored -> null));
  }

  @Override
  public CompletableFuture<Void> sendBinary(final ByteBuffer data, final boolean last) {
    Objects.requireNonNull(data, "data");
    return ready.thenCompose(ws -> ws.sendBinary(data, last).thenApply(ignored -> null));
  }

  @Override
  public CompletableFuture<Void> ping(final ByteBuffer data) {
    Objects.requireNonNull(data, "data");
    return ready.thenCompose(ws -> ws.sendPing(data).thenApply(ignored -> null));
  }

  @Override
  public CompletableFuture<Void> pong(final ByteBuffer data) {
    Objects.requireNonNull(data, "data");
    return ready.thenCompose(ws -> ws.sendPong(data).thenApply(ignored -> null));
  }

  @Override
  public CompletableFuture<Void> close(final int statusCode, final String reason) {
    validateCloseCode(statusCode);
    return ready.thenCompose(ws -> ws.sendClose(statusCode, reason).thenApply(ignored -> null));
  }

  @Override
  public boolean isOpen() {
    final WebSocket socket = delegate;
    return socket != null && !socket.isOutputClosed();
  }

  @Override
  public String subprotocol() {
    final WebSocket socket = delegate;
    return socket == null ? "" : socket.getSubprotocol();
  }
}
