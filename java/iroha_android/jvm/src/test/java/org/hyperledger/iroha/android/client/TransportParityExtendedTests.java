package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.net.InetAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import java.io.IOException;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.Headers;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okio.ByteString;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;
import org.hyperledger.iroha.android.client.websocket.JdkWebSocketConnector;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.junit.AfterClass;
import org.junit.Test;

/** Parity tests covering OkHttp vs JDK transports for streaming surfaces on the JVM. */
public final class TransportParityExtendedTests {

  private static final int TEST_TIMEOUT_SECONDS = 5;
  private static final OkHttpClient SHARED_CLIENT = new OkHttpClient();
  private static final ExecutorService JDK_EXECUTOR =
      Executors.newCachedThreadPool(
          runnable -> {
            final Thread thread = new Thread(runnable, "jvm-http-executor");
            thread.setDaemon(true);
            return thread;
          });
  private static final HttpClient JDK_CLIENT =
      HttpClient.newBuilder()
          .executor(JDK_EXECUTOR)
          .connectTimeout(Duration.ofSeconds(5))
          .version(HttpClient.Version.HTTP_1_1)
          .build();

  @AfterClass
  public static void shutdownOkHttp() {
    SHARED_CLIENT.dispatcher().executorService().shutdownNow();
    SHARED_CLIENT.connectionPool().evictAll();
    if (SHARED_CLIENT.cache() != null) {
      try {
        SHARED_CLIENT.cache().close();
      } catch (final IOException ignored) {
        // Best-effort cleanup for test shutdown.
      }
    }
  }

  @AfterClass
  public static void shutdownJdkExecutor() {
    JDK_EXECUTOR.shutdownNow();
  }

  @Test
  public void sseParityAcrossOkHttpAndJdkExecutors() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final String body =
          String.join(
              "\n",
              "id: 7",
              "event: update",
              "data: hello",
              "",
              "data: keepalive",
              "");
      final MockResponse response =
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "text/event-stream")
              .setBody(body);
      server.enqueue(response);
      server.enqueue(response);
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final URI baseUri = new URI(server.url("/").toString());
      final ToriiEventStreamOptions options = ToriiEventStreamOptions.defaultOptions();

      final RecordingSseListener okHttpListener = new RecordingSseListener();
      final ToriiEventStreamClient okHttpClient =
          ToriiEventStreamClient.builder()
              .setBaseUri(baseUri)
              .setTransportExecutor(new OkHttpTestExecutor(SHARED_CLIENT))
              .build();
      try (ToriiEventStream stream =
          okHttpClient.openSseStream("/events", options, okHttpListener)) {
        stream.completion().get(TEST_TIMEOUT_SECONDS, TimeUnit.SECONDS);
      }

      final RecordingSseListener jdkListener = new RecordingSseListener();
      final ToriiEventStreamClient jdkClient =
          ToriiEventStreamClient.builder()
              .setBaseUri(baseUri)
              .setTransportExecutor(new JavaHttpExecutor(JDK_CLIENT))
              .build();
      try (ToriiEventStream stream =
          jdkClient.openSseStream("/events", options, jdkListener)) {
        stream.completion().get(TEST_TIMEOUT_SECONDS, TimeUnit.SECONDS);
      }

      assertEquals(okHttpListener.events.size(), jdkListener.events.size());
      for (int i = 0; i < okHttpListener.events.size(); i++) {
        final ServerSentEvent left = okHttpListener.events.get(i);
        final ServerSentEvent right = jdkListener.events.get(i);
        assertEquals("event mismatch at index " + i, left.event(), right.event());
        assertEquals("data mismatch at index " + i, left.data(), right.data());
        assertEquals("id mismatch at index " + i, left.id(), right.id());
      }
    }
  }

  @Test
  public void webSocketParityAcrossOkHttpAndJdkConnectors() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().withWebSocketUpgrade(new ScriptedWebSocket()));
      server.enqueue(new MockResponse().withWebSocketUpgrade(new ScriptedWebSocket()));
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final URI baseUri = new URI(server.url("/").toString());
      final ToriiWebSocketOptions options =
          ToriiWebSocketOptions.builder()
              .setConnectTimeout(Duration.ofSeconds(TEST_TIMEOUT_SECONDS))
              .build();

      final RecordingWebSocketListener okHttpListener = new RecordingWebSocketListener();
      final ToriiWebSocketClient okHttpClient =
          ToriiWebSocketClient.builder()
              .setBaseUri(baseUri)
              .setWebSocketConnector(new OkHttpWebSocketTestConnector(SHARED_CLIENT))
              .build();
      final ToriiWebSocketSession okHttpSession =
          okHttpClient.connect("/ws", options, okHttpListener);
      assertTrue("okhttp open", okHttpListener.awaitOpen());
      awaitCloseOrCloseSession(okHttpSession, okHttpListener);

      final RecordingWebSocketListener jdkListener = new RecordingWebSocketListener();
      final ToriiWebSocketClient jdkClient =
          ToriiWebSocketClient.builder()
              .setBaseUri(baseUri)
              .setWebSocketConnector(new JdkWebSocketConnector(JDK_CLIENT))
              .build();
      final ToriiWebSocketSession jdkSession = jdkClient.connect("/ws", options, jdkListener);
      assertTrue("jdk open", jdkListener.awaitOpen());
      awaitCloseOrCloseSession(jdkSession, jdkListener);

      assertEquals(okHttpListener.textMessages, jdkListener.textMessages);
      assertEquals(okHttpListener.binaryMessages, jdkListener.binaryMessages);
    }
  }

  private static void awaitCloseOrCloseSession(
      final ToriiWebSocketSession session, final RecordingWebSocketListener listener)
      throws InterruptedException, ExecutionException, java.util.concurrent.TimeoutException {
    if (listener.awaitClose()) {
      return;
    }
    try {
      session
          .close(ToriiWebSocketSession.NORMAL_CLOSURE, "done")
          .get(TEST_TIMEOUT_SECONDS, TimeUnit.SECONDS);
    } catch (ExecutionException ex) {
      if (!(ex.getCause() instanceof IOException)) {
        throw ex;
      }
    }
    assertTrue("close", listener.awaitClose());
  }

  private static final class RecordingSseListener implements ToriiEventStreamListener {
    final List<ServerSentEvent> events = new ArrayList<>();

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
    }
  }

  private static final class RecordingWebSocketListener implements ToriiWebSocketListener {
    final List<String> textMessages = new ArrayList<>();
    final List<List<Byte>> binaryMessages = new ArrayList<>();
    final CountDownLatch closed = new CountDownLatch(1);
    final CountDownLatch opened = new CountDownLatch(1);

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      opened.countDown();
    }

    @Override
    public void onText(
        final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
      textMessages.add(data.toString());
    }

    @Override
    public void onBinary(
        final ToriiWebSocketSession session, final ByteBuffer data, final boolean last) {
      final ByteBuffer copy = data.asReadOnlyBuffer();
      final List<Byte> bytes = new ArrayList<>();
      while (copy.hasRemaining()) {
        bytes.add(copy.get());
      }
      binaryMessages.add(bytes);
    }

    @Override
    public void onClose(
        final ToriiWebSocketSession session, final int statusCode, final String reason) {
      closed.countDown();
    }

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {
      closed.countDown();
    }

    boolean awaitOpen() throws InterruptedException {
      return opened.await(TEST_TIMEOUT_SECONDS, TimeUnit.SECONDS);
    }

    boolean awaitClose() throws InterruptedException {
      return closed.await(TEST_TIMEOUT_SECONDS, TimeUnit.SECONDS);
    }
  }

  private static final class ScriptedWebSocket extends WebSocketListener {
    @Override
    public void onOpen(final WebSocket webSocket, final okhttp3.Response response) {
      webSocket.send("hello");
      webSocket.send(ByteString.of(new byte[] {0x01, 0x02, 0x03}));
      webSocket.close(1000, "complete");
    }
  }

  private static final class OkHttpTestExecutor implements HttpTransportExecutor {
    private final OkHttpClient client;

    OkHttpTestExecutor(final OkHttpClient client) {
      this.client = client;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final Request.Builder builder = new Request.Builder().url(request.uri().toString());
      final Headers.Builder headers = new Headers.Builder();
      request
          .headers()
          .forEach(
              (name, values) -> {
                for (final String value : values) {
                  headers.add(name, value);
                }
              });
      builder.headers(headers.build());
      builder.method(request.method(), buildRequestBody(request));

      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      final Call call = client.newCall(builder.build());
      call.enqueue(
          new Callback() {
            @Override
            public void onFailure(final Call call, final IOException e) {
              future.completeExceptionally(e);
            }

            @Override
            public void onResponse(final Call call, final Response response) {
              try (response) {
                final byte[] responseBody =
                    response.body() == null ? new byte[0] : response.body().bytes();
                future.complete(
                    new TransportResponse(
                        response.code(),
                        responseBody,
                        response.message(),
                        response.headers().toMultimap()));
              } catch (final IOException e) {
                future.completeExceptionally(e);
              }
            }
          });
      future.whenComplete(
          (ignored, throwable) -> {
            if (future.isCancelled()) {
              call.cancel();
            }
          });
      return future;
    }

    private static RequestBody buildRequestBody(final TransportRequest request) {
      final byte[] payload = request.body();
      if (payload == null || payload.length == 0) {
        final String method = request.method() == null ? "" : request.method().trim().toUpperCase();
        return ("GET".equals(method) || "HEAD".equals(method))
            ? null
            : RequestBody.create(new byte[0], null);
      }
      return RequestBody.create(payload, null);
    }
  }

  private static final class OkHttpWebSocketTestConnector
      implements ToriiWebSocketClient.WebSocketConnector {

    private final OkHttpClient client;

    OkHttpWebSocketTestConnector(final OkHttpClient client) {
      this.client = client;
    }

    @Override
    public CompletableFuture<TransportWebSocket> connect(
        final TransportRequest request,
        final ToriiWebSocketOptions options,
        final TransportWebSocket.Listener listener,
        final Map<String, String> defaultHeaders) {
      final CompletableFuture<TransportWebSocket> ready = new CompletableFuture<>();
      final OkHttpTransportTestWebSocket socket =
          new OkHttpTransportTestWebSocket(listener, ready);
      final Request.Builder builder = new Request.Builder().url(request.uri().toString());
      defaultHeaders.forEach(builder::addHeader);
      options.headers().forEach(builder::addHeader);
      if (!options.subprotocols().isEmpty()) {
        builder.addHeader("Sec-WebSocket-Protocol", String.join(",", options.subprotocols()));
      }
      client.newWebSocket(builder.build(), socket);
      return ready;
    }
  }

  private static final class OkHttpTransportTestWebSocket extends WebSocketListener
      implements TransportWebSocket {

    private final TransportWebSocket.Listener listener;
    private final CompletableFuture<TransportWebSocket> ready;
    private final AtomicReference<WebSocket> delegate = new AtomicReference<>(null);
    private final AtomicBoolean closed = new AtomicBoolean(false);
    private final AtomicBoolean closeNotified = new AtomicBoolean(false);
    private volatile String subprotocol = "";

    OkHttpTransportTestWebSocket(
        final TransportWebSocket.Listener listener,
        final CompletableFuture<TransportWebSocket> ready) {
      this.listener = listener;
      this.ready = ready;
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
      final WebSocket ws = requireDelegate();
      final boolean sent = ws.close(statusCode, reason == null ? "" : reason);
      if (sent) {
        closed.set(true);
        return CompletableFuture.completedFuture(null);
      }
      return failedFuture("close");
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
}
