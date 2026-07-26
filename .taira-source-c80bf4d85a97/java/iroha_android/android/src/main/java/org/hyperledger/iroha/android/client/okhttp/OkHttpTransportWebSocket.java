package org.hyperledger.iroha.android.client.okhttp;

import java.nio.ByteBuffer;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;

/**
 * OkHttp-backed {@link TransportWebSocket} used for Android builds to avoid depending on
 * {@code java.net.http}.
 */
final class OkHttpTransportWebSocket extends WebSocketListener implements TransportWebSocket {

  private final TransportWebSocket.Listener listener;
  private final CompletableFuture<TransportWebSocket> ready;
  private final AtomicReference<WebSocket> delegate = new AtomicReference<>(null);
  private final AtomicBoolean closed = new AtomicBoolean(false);
  private final AtomicBoolean closeNotified = new AtomicBoolean(false);
  private volatile String subprotocol = "";

  OkHttpTransportWebSocket(
      final TransportWebSocket.Listener listener, final CompletableFuture<TransportWebSocket> ready) {
    this.listener = Objects.requireNonNull(listener, "listener");
    this.ready = Objects.requireNonNull(ready, "ready");
  }

  private WebSocket requireDelegate() {
    final WebSocket ws = delegate.get();
    if (ws == null) {
      throw new IllegalStateException("WebSocket delegate not attached yet");
    }
    return ws;
  }

  @Override
  public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
    final boolean sent = requireDelegate().send(data.toString());
    return sent ? CompletableFuture.completedFuture(null) : failedFuture("sendText");
  }

  @Override
  public CompletableFuture<Void> sendBinary(final ByteBuffer data, final boolean last) {
    final ByteBuffer copy = data.asReadOnlyBuffer();
    final byte[] bytes = new byte[copy.remaining()];
    copy.get(bytes);
    final boolean sent = requireDelegate().send(ByteString.of(bytes));
    return sent ? CompletableFuture.completedFuture(null) : failedFuture("sendBinary");
  }

  @Override
  public CompletableFuture<Void> close(final int statusCode, final String reason) {
    validateCloseCode(statusCode);
    if (closed.get()) {
      return CompletableFuture.completedFuture(null);
    }
    final WebSocket ws = requireDelegate();
    ws.close(statusCode, reason == null ? "" : reason);
    closed.set(true);
    return CompletableFuture.completedFuture(null);
  }

  @Override
  public boolean isOpen() {
    return delegate.get() != null && !closed.get();
  }

  @Override
  public String subprotocol() {
    return subprotocol;
  }

  @Override
  public void onOpen(final WebSocket webSocket, final Response response) {
    delegate.set(webSocket);
    subprotocol = response != null ? response.header("Sec-WebSocket-Protocol", "") : "";
    listener.onOpen(this);
    ready.complete(this);
  }

  @Override
  public void onMessage(final WebSocket webSocket, final String text) {
    listener.onText(this, text, true);
  }

  @Override
  public void onMessage(final WebSocket webSocket, final ByteString bytes) {
    listener.onBinary(this, ByteBuffer.wrap(bytes.toByteArray()), true);
  }

  @Override
  public void onClosing(final WebSocket webSocket, final int code, final String reason) {
    closed.set(true);
    if (closeNotified.compareAndSet(false, true)) {
      listener.onClose(this, code, reason);
    }
  }

  @Override
  public void onClosed(final WebSocket webSocket, final int code, final String reason) {
    closed.set(true);
    if (closeNotified.compareAndSet(false, true)) {
      listener.onClose(this, code, reason);
    }
  }

  @Override
  public void onFailure(final WebSocket webSocket, final Throwable t, final Response response) {
    closed.set(true);
    if (!ready.isDone()) {
      ready.completeExceptionally(t);
    }
    listener.onError(this, t);
  }

  private static CompletableFuture<Void> failedFuture(final String operation) {
    final CompletableFuture<Void> failed = new CompletableFuture<>();
    failed.completeExceptionally(
        new IllegalStateException(operation + " failed because the WebSocket is closed"));
    return failed;
  }
}
