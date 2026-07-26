package org.hyperledger.iroha.android.client.websocket;

import java.net.http.WebSocket;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;
import org.junit.Test;

public final class JavaTransportWebSocketTests {

  @Test
  public void forwardsCallbacksAndDelegatesSends() {
    final RecordingListener listener = new RecordingListener();
    final JavaTransportWebSocket socket = new JavaTransportWebSocket(listener);
    final FakeWebSocket delegate = new FakeWebSocket();
    socket.attach(delegate);

    socket.onText(delegate, "hello", true);
    socket.onBinary(delegate, ByteBuffer.wrap(new byte[] {0x01}), true);
    socket.onPing(delegate, ByteBuffer.wrap(new byte[] {0x02}));
    socket.onPong(delegate, ByteBuffer.wrap(new byte[] {0x03}));
    socket.onClose(delegate, 1000, "bye");
    socket.onError(delegate, new IllegalStateException("boom"));

    socket.sendText("payload", true).join();
    socket.sendBinary(ByteBuffer.wrap(new byte[] {0x04}), true).join();
    socket.ping(ByteBuffer.wrap(new byte[] {0x05})).join();
    socket.pong(ByteBuffer.wrap(new byte[] {0x06})).join();
    socket.close(1000, "closed").join();

    assert listener.text.size() == 1 : "text callback should fire";
    assert listener.binary.size() == 1 : "binary callback should fire";
    assert listener.pings == 1 : "ping callback should fire";
    assert listener.pongs == 1 : "pong callback should fire";
    assert listener.closed == 1 : "close callback should fire";
    assert listener.errors == 1 : "error callback should fire";

    assert delegate.textSent == 1 : "text send should delegate";
    assert delegate.binarySent == 1 : "binary send should delegate";
    assert delegate.pings == 1 : "ping send should delegate";
    assert delegate.pongs == 1 : "pong send should delegate";
    assert delegate.closes == 1 : "close send should delegate";
  }

  private static final class RecordingListener implements TransportWebSocket.Listener {
    final List<String> text = new ArrayList<>();
    final List<ByteBuffer> binary = new ArrayList<>();
    int pings = 0;
    int pongs = 0;
    int errors = 0;
    int closed = 0;

    @Override
    public void onText(final TransportWebSocket socket, final CharSequence data, final boolean last) {
      text.add(data.toString());
    }

    @Override
    public void onBinary(final TransportWebSocket socket, final ByteBuffer data, final boolean last) {
      binary.add(data);
    }

    @Override
    public void onPing(final TransportWebSocket socket, final ByteBuffer data) {
      pings++;
    }

    @Override
    public void onPong(final TransportWebSocket socket, final ByteBuffer data) {
      pongs++;
    }

    @Override
    public void onError(final TransportWebSocket socket, final Throwable error) {
      errors++;
    }

    @Override
    public void onClose(final TransportWebSocket socket, final int statusCode, final String reason) {
      closed++;
    }
  }

  private static final class FakeWebSocket implements WebSocket {
    int textSent = 0;
    int binarySent = 0;
    int pings = 0;
    int pongs = 0;
    int closes = 0;

    @Override
    public CompletableFuture<WebSocket> sendText(final CharSequence data, final boolean last) {
      textSent++;
      return CompletableFuture.completedFuture(this);
    }

    @Override
    public CompletableFuture<WebSocket> sendBinary(final ByteBuffer data, final boolean last) {
      binarySent++;
      return CompletableFuture.completedFuture(this);
    }

    @Override
    public CompletableFuture<WebSocket> sendPing(final ByteBuffer message) {
      pings++;
      return CompletableFuture.completedFuture(this);
    }

    @Override
    public CompletableFuture<WebSocket> sendPong(final ByteBuffer message) {
      pongs++;
      return CompletableFuture.completedFuture(this);
    }

    @Override
    public CompletableFuture<WebSocket> sendClose(final int statusCode, final String reason) {
      closes++;
      return CompletableFuture.completedFuture(this);
    }

    @Override
    public void request(final long n) {
      // No-op for tests.
    }

    @Override
    public String getSubprotocol() {
      return "";
    }

    @Override
    public boolean isInputClosed() {
      return false;
    }

    @Override
    public boolean isOutputClosed() {
      return false;
    }

    @Override
    public void abort() {
      // No-op for tests.
    }
  }
}
